/**
 * Shared helpers for SilverOak Playwright specs.
 *
 * These cover the small surface the specs need:
 *   - sign in via the actual UI (not a backend shortcut) so we validate the
 *     login page, session cookie propagation, and post-login redirect.
 *   - direct API probe via `request.newContext()` to assert server-side state
 *     after a UI-driven action — the whole point of fullstack E2E.
 */

import { expect, type APIRequestContext, type Page, type Browser } from "@playwright/test";

export const DEMO_PASSWORD = "ChangeMeNow!2025";

/** Known demo accounts seeded by `infrastructure::seed`. */
export const DEMO_USERS = {
  admin: "admin@silveroak.local",
  medical: "medical@silveroak.local",
  approver: "approver@silveroak.local",
  finance: "finance@silveroak.local",
  senior: "senior@silveroak.local",
  family: "family@silveroak.local",
} as const;

/**
 * Sign in through the real login form and confirm the post-login redirect
 * landed us somewhere other than `/login`.  Throws if login fails so the
 * spec fails fast with a clear message.
 */
export async function signInViaUI(page: Page, email: string, password = DEMO_PASSWORD) {
  await page.goto("/login");
  // Login form controls are keyed by labels/placeholders in `pages/login.rs`.
  // Fall back to type=email/password selectors which always work regardless of label text.
  await page.locator('input[type="email"], input[name="email"]').fill(email);
  await page.locator('input[type="password"], input[name="password"]').fill(password);
  await Promise.all([
    page.waitForURL((url) => !url.pathname.endsWith("/login"), { timeout: 15_000 }),
    page.locator('button[type="submit"], button:has-text("Sign in"), button:has-text("Log in")').first().click(),
  ]);
  expect(page.url(), `post-login redirect for ${email}`).not.toMatch(/\/login\/?$/);
}

/** Log the current user out by clicking the sign-out control, if visible. */
export async function signOut(page: Page) {
  const maybe = page.locator('button:has-text("Sign out"), button:has-text("Logout")');
  if (await maybe.first().isVisible().catch(() => false)) {
    await maybe.first().click();
    await page.waitForURL(/\/login\/?$/, { timeout: 10_000 });
  }
}

/**
 * Direct HTTPS login — used to fetch an api_token + signing_key for making
 * signed API calls in assertions that need to inspect server-side state.
 * Uses the proxy at `/api/v1/auth/login` so it mirrors how the real SPA logs in.
 */
export async function apiLogin(
  request: APIRequestContext,
  email = DEMO_USERS.admin,
  password = DEMO_PASSWORD,
): Promise<{ api_token: string; signing_key: string; user_id: string }> {
  const res = await request.post("/api/v1/auth/login", {
    data: { email, password },
  });
  if (!res.ok()) {
    throw new Error(`login failed for ${email}: ${res.status()} ${await res.text()}`);
  }
  return (await res.json()) as { api_token: string; signing_key: string; user_id: string };
}

/** HMAC-SHA256 of the canonical 4-field string, hex-encoded. */
export async function hmacSha256Hex(key: string, canonical: string): Promise<string> {
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

/** SHA-256 of the body bytes, hex-encoded.  Matches backend canonical hash. */
export async function sha256Hex(body: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body));
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/** Issue a signed GET against the real API to probe server-side state. */
export async function signedGet(
  request: APIRequestContext,
  path: string,
  creds: { api_token: string; signing_key: string },
): Promise<Response> {
  const ts = Math.floor(Date.now() / 1000).toString();
  const nonce = `e2e-${ts}-${Math.random().toString(36).slice(2)}`;
  const signPath = path.split("?")[0];
  const canonical = `GET\n${signPath}\n${ts}\n${nonce}`;
  const sig = await hmacSha256Hex(creds.signing_key, canonical);
  const bodyHash = await sha256Hex("");
  const res = await request.get(path, {
    headers: {
      "x-silveroak-token": creds.api_token,
      "x-silveroak-timestamp": ts,
      "x-silveroak-nonce": nonce,
      "x-silveroak-signature": sig,
      "x-silveroak-body-hash": bodyHash,
    },
  });
  return res as unknown as Response;
}

/**
 * Wait for the SPA to finish its initial auth bootstrap — the guard renders a
 * "Loading…" state until `AuthAction::Bootstrap` completes.  Most pages call
 * `fetch()` shortly after, so we also wait for network idleness.
 */
export async function waitForAppReady(page: Page) {
  await page.waitForLoadState("domcontentloaded");
  await page.waitForLoadState("networkidle").catch(() => {/* proxy keeps conns alive; best-effort */});
}

export { type Browser };
