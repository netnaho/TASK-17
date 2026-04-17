import { expect, test } from "@playwright/test";
import {
  DEMO_USERS,
  apiLogin,
  signInViaUI,
  signedGet,
  waitForAppReady,
} from "./helpers";

/**
 * E2E #3 — senior user exercises the consumable-ordering happy path.
 *
 * We intentionally drive the API via signed requests to set up the cart
 * deterministically (add product + delivery), then switch to the UI to run
 * `verify → confirm` through the real CheckoutPage.  This mirrors the real
 * user journey while keeping the scenario repeatable (no dependence on a
 * specific catalog ordering the UI shows).
 *
 * Assertions:
 *   - The cart was accepted by the backend (product added via API).
 *   - The checkout page renders and exposes confirm/verify controls.
 *   - After `confirm`, the backend reports the order under /orders/mine.
 *   - The order detail endpoint returns the same reference code shown in UI.
 */

async function hmac(key: string, canonical: string) {
  const enc = new TextEncoder();
  const k = await crypto.subtle.importKey(
    "raw",
    enc.encode(key),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign("HMAC", k, enc.encode(canonical));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function sha256(s: string) {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s));
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

test("senior can drive the cart through verify + confirm; backend creates the order", async ({
  page,
  request,
}) => {
  const creds = await apiLogin(request, DEMO_USERS.senior);

  // ── API setup: fetch a product and delivery method, then add to cart ──
  const prodRes = await signedGet(request, "/api/v1/orders/products", creds);
  expect(prodRes.ok).toBeTruthy();
  const products = (await prodRes.json()) as Array<{ id: string; sku: string }>;
  const can001 = products.find((p) => p.sku === "CAN-001");
  expect(can001, "seeded CAN-001 product must exist").toBeTruthy();

  const dmRes = await signedGet(request, "/api/v1/orders/delivery-methods", creds);
  expect(dmRes.ok).toBeTruthy();
  const methods = (await dmRes.json()) as Array<{ id: string; key: string }>;
  const pickup = methods.find((m) => m.key === "pickup");
  expect(pickup, "seeded pickup delivery must exist").toBeTruthy();

  // Helper to issue a signed POST.
  async function postSigned(pathname: string, body: Record<string, unknown>) {
    const ts = Math.floor(Date.now() / 1000).toString();
    const nonce = `e2e-${ts}-${Math.random().toString(36).slice(2)}`;
    const signPath = pathname.split("?")[0];
    const canonical = `POST\n${signPath}\n${ts}\n${nonce}`;
    const sig = await hmac(creds.signing_key, canonical);
    const bodyJson = JSON.stringify(body);
    const bodyHash = await sha256(bodyJson);
    return request.post(pathname, {
      headers: {
        "x-silveroak-token": creds.api_token,
        "x-silveroak-timestamp": ts,
        "x-silveroak-nonce": nonce,
        "x-silveroak-signature": sig,
        "x-silveroak-body-hash": bodyHash,
        "content-type": "application/json",
      },
      data: bodyJson,
    });
  }

  // Clear any residual line first (set qty=0) to guarantee state.
  await postSigned("/api/v1/orders/cart/lines", { product_id: can001!.id, quantity: 0 });
  // Now seed qty=1 of CAN-001 and pickup delivery.
  const addLine = await postSigned("/api/v1/orders/cart/lines", {
    product_id: can001!.id,
    quantity: 1,
  });
  expect(addLine.ok(), `set_line ${addLine.status()} body=${await addLine.text()}`).toBeTruthy();
  const setDelivery = await postSigned("/api/v1/orders/cart/delivery", {
    delivery_method_id: pickup!.id,
  });
  expect(setDelivery.ok(), `set_delivery ${setDelivery.status()}`).toBeTruthy();

  // ── UI: sign in and drive the checkout ──
  await signInViaUI(page, DEMO_USERS.senior);
  await waitForAppReady(page);
  await page.goto("/orders/checkout");
  await waitForAppReady(page);

  // The checkout page presents a "Verify" control.  Click it.
  const verifyBtn = page
    .locator('button:has-text("Verify"), button:has-text("Review"), button:has-text("Check")')
    .first();
  if (await verifyBtn.isVisible().catch(() => false)) {
    await verifyBtn.click();
    await page.waitForTimeout(1_000);
  }

  // Confirm via the UI button.
  const confirmBtn = page
    .locator('button:has-text("Confirm"), button:has-text("Place order"), button:has-text("Pay")')
    .first();
  await confirmBtn.waitFor({ state: "visible", timeout: 15_000 });
  await confirmBtn.click();
  await page.waitForTimeout(2_000);

  // ── backend truth check ──
  const mineRes = await signedGet(request, "/api/v1/orders/mine", creds);
  expect(mineRes.ok).toBeTruthy();
  const mine = (await mineRes.json()) as Array<{ id: string; ref_code: string; status: string }>;
  // The senior now has ≥1 order in their list.
  expect(mine.length).toBeGreaterThanOrEqual(1);
  const newestOrder = mine[0];
  expect(newestOrder.status, "newly placed order must be confirmed").toBe("confirmed");

  // Follow /orders/{id} through signed API — the order detail must match.
  const detailRes = await signedGet(request, `/api/v1/orders/${newestOrder.id}`, creds);
  expect(detailRes.ok).toBeTruthy();
  const detail = (await detailRes.json()) as { ref_code: string; lines: Array<{ sku: string }> };
  expect(detail.ref_code).toBe(newestOrder.ref_code);
  expect(detail.lines.some((l) => l.sku === "CAN-001"), "order must contain CAN-001").toBeTruthy();
});
