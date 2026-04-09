use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::Principal;

use super::pricing::{price_cart, CartInput, PricingSummary, daily_limit_per_sku};

// ── snapshot TTL -----------------------------------------------------------
const SNAPSHOT_TTL_SECS: i64 = 600; // 10 minutes

// ── DTOs ------------------------------------------------------------------
#[derive(Debug, Serialize)]
pub struct ProductItem {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub store_key: String,
    pub base_price_cents: i64,
    pub stock: i32,
}

#[derive(Debug, Serialize)]
pub struct DeliveryMethod {
    pub id: Uuid,
    pub key: String,
    pub label: String,
    pub fee_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct CartView {
    pub id: Uuid,
    pub lines: Vec<CartLineView>,
    pub coupon_code: Option<String>,
    pub delivery_method_id: Option<Uuid>,
    pub has_cross_store: bool,
    pub pricing: Option<PricingSummary>,
}

#[derive(Debug, Serialize)]
pub struct CartLineView {
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub store_key: String,
}

#[derive(Debug, Serialize)]
pub struct OrderSummary {
    pub id: Uuid,
    pub ref_code: String,
    pub status: String,
    pub total_cents: i64,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct OrderDetail {
    pub id: Uuid,
    pub ref_code: String,
    pub status: String,
    pub subtotal_cents: i64,
    pub delivery_fee_cents: i64,
    pub discount_cents: i64,
    pub total_cents: i64,
    pub delivery_key: String,
    pub coupon_code: Option<String>,
    pub lines: Vec<OrderLineView>,
    pub adjustments: Vec<AdjustmentView>,
    pub confirmed_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct OrderLineView {
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
    pub member_price_applied: bool,
    pub threshold_discount_applied: bool,
    pub discount_bps: i32,
}

#[derive(Debug, Serialize)]
pub struct AdjustmentView {
    pub kind: String,
    pub label: String,
    pub amount_cents: i64,
}

#[derive(Debug, Deserialize)]
pub struct SetLineInput {
    pub product_id: Uuid,
    pub quantity: i32,   // 0 = remove
}

#[derive(Debug, Deserialize)]
pub struct ApplyCouponInput {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct SetDeliveryInput {
    pub delivery_method_id: Uuid,
}

// ── internal helpers -------------------------------------------------------
async fn active_cart_id(pool: &PgPool, user_id: Uuid) -> Result<Uuid, ApiAppError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM carts WHERE user_id=$1 AND is_active=true ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    if let Some((id,)) = row {
        return Ok(id);
    }
    // Create an empty cart
    let new: (Uuid,) = sqlx::query_as(
        "INSERT INTO carts (user_id) VALUES ($1) RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(new.0)
}

async fn load_cart_view(
    pool: &PgPool,
    cart_id: Uuid,
    p: &Principal,
) -> Result<CartView, ApiAppError> {
    let cart: (Option<Uuid>, Option<Uuid>, bool) = sqlx::query_as(
        "SELECT coupon_id, delivery_method_id, has_cross_store FROM carts WHERE id=$1",
    )
    .bind(cart_id)
    .fetch_one(pool)
    .await?;

    let lines: Vec<(Uuid, String, String, i32, String)> = sqlx::query_as(
        "SELECT cl.product_id, p.sku, p.name, cl.quantity, p.store_key
         FROM cart_lines cl JOIN product_catalog p ON p.id = cl.product_id
         WHERE cl.cart_id = $1 ORDER BY p.store_key, p.sku",
    )
    .bind(cart_id)
    .fetch_all(pool)
    .await?;

    let coupon_code = if let Some(cid) = cart.0 {
        let r: Option<(String,)> = sqlx::query_as("SELECT code FROM coupons WHERE id=$1")
            .bind(cid).fetch_optional(pool).await?;
        r.map(|x| x.0)
    } else {
        None
    };

    let delivery_method_id = cart.1;

    // Price the cart if it has lines and a delivery method.
    let pricing = if !lines.is_empty() {
        let delivery_fee = if let Some(dm_id) = delivery_method_id {
            let f: Option<(i64,)> = sqlx::query_as("SELECT fee_cents FROM delivery_methods WHERE id=$1")
                .bind(dm_id).fetch_optional(pool).await?;
            f.map(|x| x.0).unwrap_or(0)
        } else {
            0
        };
        let summary = price_cart(pool, &CartInput {
            lines: lines.iter().map(|l| (l.0, l.3)).collect(),
            coupon_id: cart.0,
            delivery_fee_cents: delivery_fee,
            user_roles: p.roles.clone(),
            user_id: p.user_id,
        }).await?;
        Some(summary)
    } else {
        None
    };

    Ok(CartView {
        id: cart_id,
        lines: lines.into_iter().map(|l| CartLineView {
            product_id: l.0, sku: l.1, name: l.2, quantity: l.3, store_key: l.4,
        }).collect(),
        coupon_code,
        delivery_method_id,
        has_cross_store: cart.2,
        pricing,
    })
}

// ── use cases -------------------------------------------------------------
pub async fn list_products(pool: &PgPool) -> Result<Vec<ProductItem>, ApiAppError> {
    let rows: Vec<(Uuid, String, String, String, i64, i32)> = sqlx::query_as(
        "SELECT id, sku, name, store_key, base_price_cents, stock
         FROM product_catalog WHERE is_active=true ORDER BY store_key, sku",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| ProductItem {
        id: r.0, sku: r.1, name: r.2, store_key: r.3,
        base_price_cents: r.4, stock: r.5,
    }).collect())
}

pub async fn list_delivery_methods(pool: &PgPool) -> Result<Vec<DeliveryMethod>, ApiAppError> {
    let rows: Vec<(Uuid, String, String, i64)> = sqlx::query_as(
        "SELECT id, key, label, fee_cents FROM delivery_methods WHERE is_active=true ORDER BY fee_cents",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| DeliveryMethod {
        id: r.0, key: r.1, label: r.2, fee_cents: r.3,
    }).collect())
}

pub async fn get_cart(pool: &PgPool, p: &Principal) -> Result<CartView, ApiAppError> {
    p.require("orders:read")?;
    let cart_id = active_cart_id(pool, p.user_id).await?;
    load_cart_view(pool, cart_id, p).await
}

pub async fn set_line(
    pool: &PgPool,
    p: &Principal,
    input: SetLineInput,
) -> Result<CartView, ApiAppError> {
    p.require("orders:write")?;

    if input.quantity < 0 {
        return Err(ApiAppError::BadRequest("quantity cannot be negative".into()));
    }
    const MAX_LINE_QTY: i32 = 5;
    if input.quantity > MAX_LINE_QTY {
        return Err(ApiAppError::BadRequest(format!(
            "cannot order more than {} of any item per day", MAX_LINE_QTY
        )));
    }

    // Validate product exists + is active
    let product: Option<(String, bool)> = sqlx::query_as(
        "SELECT store_key, is_active FROM product_catalog WHERE id=$1",
    )
    .bind(input.product_id)
    .fetch_optional(pool)
    .await?;
    let (store_key, is_active) = product
        .ok_or_else(|| ApiAppError::BadRequest("product not found".into()))?;
    if !is_active {
        return Err(ApiAppError::BadRequest("product is not available".into()));
    }

    let cart_id = active_cart_id(pool, p.user_id).await?;

    // Detect cross-store: store_key is nullable (TEXT) in the carts table
    let existing_store: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT store_key FROM carts WHERE id=$1",
    )
    .bind(cart_id)
    .fetch_optional(pool)
    .await?;

    let mut tx = pool.begin().await?;
    if input.quantity == 0 {
        sqlx::query("DELETE FROM cart_lines WHERE cart_id=$1 AND product_id=$2")
            .bind(cart_id).bind(input.product_id)
            .execute(&mut *tx).await?;
    } else {
        sqlx::query(
            "INSERT INTO cart_lines (cart_id, product_id, quantity)
             VALUES ($1,$2,$3)
             ON CONFLICT (cart_id, product_id) DO UPDATE SET quantity=EXCLUDED.quantity",
        )
        .bind(cart_id).bind(input.product_id).bind(input.quantity)
        .execute(&mut *tx).await?;
    }

    // Update cart store_key and cross-store flag
    let cart_store: Option<String> = existing_store.and_then(|r| r.0);
    let cross_store = cart_store.as_deref().map(|s| s != store_key).unwrap_or(false);

    // Determine the canonical store_key for the cart (from remaining lines)
    let first_store: Option<(String,)> = sqlx::query_as(
        "SELECT p.store_key FROM cart_lines cl
         JOIN product_catalog p ON p.id=cl.product_id
         WHERE cl.cart_id=$1 ORDER BY cl.id LIMIT 1",
    )
    .bind(cart_id)
    .fetch_optional(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE carts SET store_key=$2, has_cross_store=$3, updated_at=NOW() WHERE id=$1",
    )
    .bind(cart_id)
    .bind(first_store.map(|s| s.0))
    .bind(cross_store)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    load_cart_view(pool, cart_id, p).await
}

pub async fn apply_coupon(
    pool: &PgPool,
    p: &Principal,
    input: ApplyCouponInput,
) -> Result<CartView, ApiAppError> {
    p.require("orders:write")?;
    let coupon: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT id, is_active FROM coupons WHERE code=$1",
    )
    .bind(&input.code)
    .fetch_optional(pool)
    .await?;
    let (coupon_id, is_active) = coupon
        .ok_or_else(|| ApiAppError::BadRequest("coupon code not found".into()))?;
    if !is_active {
        return Err(ApiAppError::BadRequest("coupon is not active".into()));
    }
    let cart_id = active_cart_id(pool, p.user_id).await?;
    sqlx::query("UPDATE carts SET coupon_id=$2, updated_at=NOW() WHERE id=$1")
        .bind(cart_id).bind(coupon_id)
        .execute(pool).await?;
    load_cart_view(pool, cart_id, p).await
}

pub async fn remove_coupon(pool: &PgPool, p: &Principal) -> Result<CartView, ApiAppError> {
    p.require("orders:write")?;
    let cart_id = active_cart_id(pool, p.user_id).await?;
    sqlx::query("UPDATE carts SET coupon_id=NULL, updated_at=NOW() WHERE id=$1")
        .bind(cart_id).execute(pool).await?;
    load_cart_view(pool, cart_id, p).await
}

pub async fn set_delivery(
    pool: &PgPool,
    p: &Principal,
    input: SetDeliveryInput,
) -> Result<CartView, ApiAppError> {
    p.require("orders:write")?;
    // Validate delivery method
    let dm: Option<(bool,)> = sqlx::query_as("SELECT is_active FROM delivery_methods WHERE id=$1")
        .bind(input.delivery_method_id).fetch_optional(pool).await?;
    if dm.map(|d| !d.0).unwrap_or(true) {
        return Err(ApiAppError::BadRequest("invalid delivery method".into()));
    }
    let cart_id = active_cart_id(pool, p.user_id).await?;
    sqlx::query("UPDATE carts SET delivery_method_id=$2, updated_at=NOW() WHERE id=$1")
        .bind(cart_id).bind(input.delivery_method_id)
        .execute(pool).await?;
    load_cart_view(pool, cart_id, p).await
}

// ── verify: re-price and save snapshot -----------------------------------
pub async fn verify_cart(pool: &PgPool, p: &Principal) -> Result<PricingSummary, ApiAppError> {
    p.require("orders:write")?;
    let cart_id = active_cart_id(pool, p.user_id).await?;

    let cart: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT coupon_id, delivery_method_id FROM carts WHERE id=$1",
    )
    .bind(cart_id)
    .fetch_one(pool)
    .await?;

    if cart.1.is_none() {
        return Err(ApiAppError::BadRequest("select a delivery method before verifying".into()));
    }

    let lines: Vec<(Uuid, i32)> = sqlx::query_as(
        "SELECT product_id, quantity FROM cart_lines WHERE cart_id=$1 ORDER BY product_id",
    )
    .bind(cart_id)
    .fetch_all(pool)
    .await?;

    if lines.is_empty() {
        return Err(ApiAppError::BadRequest("cart is empty".into()));
    }

    let delivery_fee: (i64,) = sqlx::query_as("SELECT fee_cents FROM delivery_methods WHERE id=$1")
        .bind(cart.1.unwrap()).fetch_one(pool).await?;

    let summary = price_cart(pool, &CartInput {
        lines: lines.clone(),
        coupon_id: cart.0,
        delivery_fee_cents: delivery_fee.0,
        user_roles: p.roles.clone(),
        user_id: p.user_id,
    }).await?;

    // Check stock availability
    for line in &summary.lines {
        if line.stock < line.quantity {
            return Err(ApiAppError::BadRequest(format!(
                "{}: only {} in stock, requested {}",
                line.name, line.stock, line.quantity
            )));
        }
    }

    // Enforce purchase limits during verify (not just warn)
    if !summary.purchase_limit_warnings.is_empty() {
        return Err(ApiAppError::BadRequest(format!(
            "daily limit exceeded: {}",
            summary.purchase_limit_warnings.join("; ")
        )));
    }

    // Save snapshot
    let snapshot_json = serde_json::to_value(&summary)
        .map_err(|e| ApiAppError::Internal(e.to_string()))?;
    let expires_at = Utc::now() + Duration::seconds(SNAPSHOT_TTL_SECS);

    // Invalidate old snapshots for this cart
    sqlx::query("DELETE FROM order_verification_snapshots WHERE cart_id=$1")
        .bind(cart_id).execute(pool).await?;

    sqlx::query(
        "INSERT INTO order_verification_snapshots (cart_id, user_id, snapshot_json, expires_at)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(cart_id).bind(p.user_id)
    .bind(snapshot_json).bind(expires_at)
    .execute(pool).await?;

    Ok(summary)
}

// ── confirm: transactional order creation --------------------------------
pub async fn confirm_order(pool: &PgPool, p: &Principal) -> Result<OrderDetail, ApiAppError> {
    p.require("orders:write")?;
    let cart_id = active_cart_id(pool, p.user_id).await?;

    // Verify a fresh-enough snapshot exists
    let snapshot: Option<(Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT id, snapshot_json FROM order_verification_snapshots
         WHERE cart_id=$1 AND user_id=$2 AND expires_at > NOW()
         ORDER BY verified_at DESC LIMIT 1",
    )
    .bind(cart_id).bind(p.user_id).fetch_optional(pool).await?;
    let (_snap_id, snap_json) = snapshot.ok_or_else(|| {
        ApiAppError::BadRequest("cart not verified or snapshot expired — verify first".into())
    })?;
    let summary: PricingSummary = serde_json::from_value(snap_json)
        .map_err(|e| ApiAppError::Internal(e.to_string()))?;

    let cart: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT coupon_id, delivery_method_id FROM carts WHERE id=$1",
    )
    .bind(cart_id).fetch_one(pool).await?;
    let delivery_method_id = cart.1.ok_or_else(|| {
        ApiAppError::BadRequest("delivery method not set".into())
    })?;

    let mut tx = pool.begin().await?;

    // Re-price inside the transaction to detect price changes since snapshot
    let lines_db: Vec<(Uuid, i32)> = sqlx::query_as(
        "SELECT product_id, quantity FROM cart_lines WHERE cart_id=$1 FOR UPDATE",
    )
    .bind(cart_id).fetch_all(&mut *tx).await?;

    if lines_db.is_empty() {
        return Err(ApiAppError::BadRequest("cart is empty".into()));
    }

    let delivery_fee: (i64,) = sqlx::query_as("SELECT fee_cents FROM delivery_methods WHERE id=$1")
        .bind(delivery_method_id).fetch_one(&mut *tx).await?;

    let live_summary = price_cart(pool, &CartInput {
        lines: lines_db.clone(),
        coupon_id: cart.0,
        delivery_fee_cents: delivery_fee.0,
        user_roles: p.roles.clone(),
        user_id: p.user_id,
    }).await?;

    // Price drift check
    if live_summary.total_cents != summary.total_cents {
        return Err(ApiAppError::BadRequest(format!(
            "prices changed since verification (was {}, now {}) — verify again",
            summary.total_cents, live_summary.total_cents
        )));
    }

    // Stock check + deduct (with FOR UPDATE on each product row)
    for line in &live_summary.lines {
        let stock: (i32,) = sqlx::query_as(
            "SELECT stock FROM product_catalog WHERE id=$1 FOR UPDATE",
        )
        .bind(line.product_id).fetch_one(&mut *tx).await?;
        if stock.0 < line.quantity {
            return Err(ApiAppError::BadRequest(format!(
                "{}: stock dropped to {} since verification", line.name, stock.0
            )));
        }
        // Purchase limit check
        let existing: Option<(i32,)> = sqlx::query_as(
            "SELECT quantity FROM daily_purchase_tracking
             WHERE user_id=$1 AND product_id=$2 AND purchase_date=CURRENT_DATE FOR UPDATE",
        )
        .bind(p.user_id).bind(line.product_id).fetch_optional(&mut *tx).await?;
        let already = existing.map(|r| r.0).unwrap_or(0);
        let dlimit = daily_limit_per_sku();
        if already + line.quantity > dlimit {
            return Err(ApiAppError::BadRequest(format!(
                "{}: daily limit {} exceeded (already {}, requesting {})",
                line.name, dlimit, already, line.quantity
            )));
        }
        // Deduct stock
        sqlx::query("UPDATE product_catalog SET stock = stock - $2 WHERE id=$1")
            .bind(line.product_id).bind(line.quantity)
            .execute(&mut *tx).await?;
        // Record daily purchase
        sqlx::query(
            "INSERT INTO daily_purchase_tracking (user_id, product_id, quantity)
             VALUES ($1,$2,$3)
             ON CONFLICT (user_id, product_id, purchase_date)
             DO UPDATE SET quantity = daily_purchase_tracking.quantity + EXCLUDED.quantity",
        )
        .bind(p.user_id).bind(line.product_id).bind(line.quantity)
        .execute(&mut *tx).await?;
    }

    // Increment coupon usage
    if let Some(coupon_id) = cart.0 {
        if live_summary.coupon_applied {
            sqlx::query("UPDATE coupons SET times_used = times_used + 1 WHERE id=$1")
                .bind(coupon_id).execute(&mut *tx).await?;
        }
    }

    // Build ref_code
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orders").fetch_one(&mut *tx).await?;
    let ref_code = format!("ORD-{:06}", count.0 + 1);

    // Create order
    let order_id: (Uuid,) = sqlx::query_as(
        "INSERT INTO orders
           (ref_code, user_id, delivery_method_id, coupon_id, status,
            subtotal_cents, delivery_fee_cents, discount_cents, total_cents)
         VALUES ($1,$2,$3,$4,'confirmed',$5,$6,$7,$8) RETURNING id",
    )
    .bind(&ref_code)
    .bind(p.user_id)
    .bind(delivery_method_id)
    .bind(cart.0)
    .bind(live_summary.subtotal_cents)
    .bind(live_summary.delivery_fee_cents)
    .bind(live_summary.coupon_discount_cents)
    .bind(live_summary.total_cents)
    .fetch_one(&mut *tx).await?;

    // Order lines
    for line in &live_summary.lines {
        sqlx::query(
            "INSERT INTO order_lines
               (order_id, product_id, quantity, unit_price_cents, line_total_cents,
                member_price_applied, threshold_discount_applied, discount_bps)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(order_id.0)
        .bind(line.product_id)
        .bind(line.quantity)
        .bind(line.unit_price_cents)
        .bind(line.line_total_cents)
        .bind(line.member_price_applied)
        .bind(line.threshold_discount_applied)
        .bind(line.discount_bps)
        .execute(&mut *tx).await?;
    }

    // Adjustments
    if live_summary.delivery_fee_cents > 0 {
        let dm_label: (String,) = sqlx::query_as("SELECT label FROM delivery_methods WHERE id=$1")
            .bind(delivery_method_id).fetch_one(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO order_adjustments (order_id, kind, label, amount_cents) VALUES ($1,'delivery_fee',$2,$3)",
        )
        .bind(order_id.0).bind(dm_label.0).bind(live_summary.delivery_fee_cents)
        .execute(&mut *tx).await?;
    }
    if live_summary.coupon_discount_cents > 0 {
        let clabel = live_summary.coupon_code.as_deref().unwrap_or("coupon");
        sqlx::query(
            "INSERT INTO order_adjustments (order_id, kind, label, amount_cents) VALUES ($1,'coupon',$2,$3)",
        )
        .bind(order_id.0).bind(format!("Coupon {clabel}")).bind(-live_summary.coupon_discount_cents)
        .execute(&mut *tx).await?;
    }

    // Deactivate cart + clear snapshot
    sqlx::query("UPDATE carts SET is_active=false, updated_at=NOW() WHERE id=$1")
        .bind(cart_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM order_verification_snapshots WHERE cart_id=$1")
        .bind(cart_id).execute(&mut *tx).await?;

    tx.commit().await?;
    load_order(pool, p, order_id.0).await
}

pub async fn list_my_orders(pool: &PgPool, p: &Principal) -> Result<Vec<OrderSummary>, ApiAppError> {
    p.require("orders:read")?;
    let rows: Vec<(Uuid, String, String, i64, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, ref_code, status, total_cents, created_at
         FROM orders WHERE user_id=$1 ORDER BY created_at DESC",
    )
    .bind(p.user_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| OrderSummary {
        id: r.0, ref_code: r.1, status: r.2, total_cents: r.3, created_at: r.4,
    }).collect())
}

pub async fn load_order(
    pool: &PgPool,
    p: &Principal,
    order_id: Uuid,
) -> Result<OrderDetail, ApiAppError> {
    p.require("orders:read")?;
    let ord: Option<(Uuid, String, String, Uuid, Option<Uuid>, i64, i64, i64, i64, chrono::DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT id, ref_code, status, delivery_method_id, coupon_id,
                    subtotal_cents, delivery_fee_cents, discount_cents, total_cents, confirmed_at
             FROM orders WHERE id=$1",
        )
        .bind(order_id).fetch_optional(pool).await?;
    let o = ord.ok_or(ApiAppError::NotFound)?;

    // Object-level auth: owner or admin
    let owner_row: Option<(Uuid,)> = sqlx::query_as("SELECT user_id FROM orders WHERE id=$1")
        .bind(order_id).fetch_optional(pool).await?;
    let owner = owner_row.map(|r| r.0).unwrap_or(Uuid::nil());
    if p.user_id != owner && !p.permissions.contains("scope:any") {
        return Err(ApiAppError::Forbidden("not your order".into()));
    }

    let dm: (String,) = sqlx::query_as("SELECT key FROM delivery_methods WHERE id=$1")
        .bind(o.3).fetch_one(pool).await?;
    let coupon_code = if let Some(cid) = o.4 {
        let r: Option<(String,)> = sqlx::query_as("SELECT code FROM coupons WHERE id=$1")
            .bind(cid).fetch_optional(pool).await?;
        r.map(|x| x.0)
    } else {
        None
    };

    let lines: Vec<(Uuid, String, String, i32, i64, i64, bool, bool, i32)> = sqlx::query_as(
        "SELECT ol.product_id, p.sku, p.name, ol.quantity, ol.unit_price_cents, ol.line_total_cents,
                ol.member_price_applied, ol.threshold_discount_applied, ol.discount_bps
         FROM order_lines ol JOIN product_catalog p ON p.id=ol.product_id
         WHERE ol.order_id=$1 ORDER BY p.sku",
    )
    .bind(order_id).fetch_all(pool).await?;

    let adjustments: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT kind, label, amount_cents FROM order_adjustments WHERE order_id=$1 ORDER BY id",
    )
    .bind(order_id).fetch_all(pool).await?;

    Ok(OrderDetail {
        id: o.0, ref_code: o.1, status: o.2,
        subtotal_cents: o.5, delivery_fee_cents: o.6, discount_cents: o.7, total_cents: o.8,
        delivery_key: dm.0, coupon_code, confirmed_at: o.9,
        lines: lines.into_iter().map(|l| OrderLineView {
            product_id: l.0, sku: l.1, name: l.2, quantity: l.3,
            unit_price_cents: l.4, line_total_cents: l.5,
            member_price_applied: l.6, threshold_discount_applied: l.7, discount_bps: l.8,
        }).collect(),
        adjustments: adjustments.into_iter().map(|a| AdjustmentView {
            kind: a.0, label: a.1, amount_cents: a.2,
        }).collect(),
    })
}
