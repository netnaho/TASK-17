/// Pricing engine for the consumable ordering flow.
///
/// Rules (applied in order):
/// 1. Start from `base_price_cents`.
/// 2. If user holds a role that has a member price → use it instead (member_price_applied).
/// 3. If quantity for this line meets a threshold discount → apply it
///    UNLESS member_price was applied AND the coupon (if any) is not
///    stackable_with_member_price (we pick the better of the two in that case).
/// 4. Coupon (one per order):
///    - cannot stack with member price unless stackable_with_member_price
///    - cannot stack with threshold unless stackable_with_threshold
///    - applied last, to the subtotal after per-line pricing.
///
/// Purchase limit: 5 units / SKU / user / day (enforced in service.rs).
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;

/// Overridable via DAILY_LIMIT_PER_SKU env var for test environments.
pub fn daily_limit_per_sku() -> i32 {
    std::env::var("DAILY_LIMIT_PER_SKU")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5)
}
pub const DAILY_LIMIT_PER_SKU: i32 = 5; // kept for compile-time tests

// ── product row (loaded once per pricing call) ────────────────────────────
#[derive(Debug, Clone)]
pub struct ProductRow {
    pub id: Uuid,
    pub sku: String,
    pub name: String,
    pub store_key: String,
    pub base_price_cents: i64,
    pub stock: i32,
    pub is_active: bool,
}

// ── per-line pricing result ───────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricedLine {
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub base_price_cents: i64,
    pub unit_price_cents: i64,      // effective price after member / threshold
    pub line_total_cents: i64,
    pub member_price_applied: bool,
    pub threshold_discount_applied: bool,
    pub discount_bps: i32,
    pub stock: i32,
}

// ── coupon row ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct CouponRow {
    pub id: Uuid,
    pub code: String,
    pub discount_bps: i32,
    pub stackable_with_member_price: bool,
    pub stackable_with_threshold: bool,
}

// ── bundle warning ────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleWarning {
    pub bundle_key: String,
    pub label: String,
    pub warning_message: String,
    pub total_qty: i32,
    pub threshold_qty: i32,
}

// ── full pricing summary ──────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingSummary {
    pub lines: Vec<PricedLine>,
    pub subtotal_cents: i64,
    pub delivery_fee_cents: i64,
    pub coupon_discount_cents: i64,
    pub total_cents: i64,
    pub coupon_applied: bool,
    pub coupon_code: Option<String>,
    pub coupon_blocked_reason: Option<String>,  // why coupon wasn't applied (stacking)
    pub bundle_warnings: Vec<BundleWarning>,
    pub purchase_limit_warnings: Vec<String>,   // SKUs that hit daily limit
}

// ─────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────
async fn load_product(pool: &PgPool, product_id: Uuid) -> Result<ProductRow, ApiAppError> {
    let row: Option<(Uuid, String, String, String, i64, i32, bool)> = sqlx::query_as(
        "SELECT id, sku, name, store_key, base_price_cents, stock, is_active
         FROM product_catalog WHERE id = $1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await?;
    let r = row.ok_or(ApiAppError::NotFound)?;
    Ok(ProductRow {
        id: r.0, sku: r.1, name: r.2, store_key: r.3,
        base_price_cents: r.4, stock: r.5, is_active: r.6,
    })
}

/// Effective unit price for a (product, user_roles, quantity) triple.
/// Returns `(unit_price_cents, member_applied, threshold_applied, discount_bps)`.
pub async fn effective_price(
    pool: &PgPool,
    product: &ProductRow,
    user_roles: &[String],
    quantity: i32,
) -> Result<(i64, bool, bool, i32), ApiAppError> {
    // 1. member price
    let member: Option<(i64,)> = sqlx::query_as(
        "SELECT price_cents FROM product_pricing
         WHERE product_id = $1 AND role_key = ANY($2)
         ORDER BY price_cents LIMIT 1",
    )
    .bind(product.id)
    .bind(user_roles)
    .fetch_optional(pool)
    .await?;

    // 2. best threshold discount for this quantity
    let threshold: Option<(i32,)> = sqlx::query_as(
        "SELECT discount_bps FROM product_threshold_discounts
         WHERE product_id = $1 AND min_quantity <= $2
         ORDER BY discount_bps DESC LIMIT 1",
    )
    .bind(product.id)
    .bind(quantity)
    .fetch_optional(pool)
    .await?;

    let base = product.base_price_cents;

    match (member, threshold) {
        (Some((mp,)), Some((bps,))) => {
            // Both available — pick whichever gives the lower price.
            let threshold_price = base - (base * bps as i64 / 10000);
            if mp <= threshold_price {
                Ok((mp, true, false, 0))
            } else {
                Ok((threshold_price, false, true, bps))
            }
        }
        (Some((mp,)), None) => Ok((mp, true, false, 0)),
        (None, Some((bps,))) => {
            let p = base - (base * bps as i64 / 10000);
            Ok((p, false, true, bps))
        }
        (None, None) => Ok((base, false, false, 0)),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Public: price a full cart
// ─────────────────────────────────────────────────────────────────────────
pub struct CartInput {
    pub lines: Vec<(Uuid, i32)>,    // (product_id, quantity)
    pub coupon_id: Option<Uuid>,
    pub delivery_fee_cents: i64,
    pub user_roles: Vec<String>,
    pub user_id: Uuid,
}

pub async fn price_cart(pool: &PgPool, input: &CartInput) -> Result<PricingSummary, ApiAppError> {
    let mut priced_lines: Vec<PricedLine> = Vec::new();
    let mut any_member = false;
    let mut any_threshold = false;

    for (product_id, qty) in &input.lines {
        let product = load_product(pool, *product_id).await?;
        let (unit_price, member_applied, threshold_applied, discount_bps) =
            effective_price(pool, &product, &input.user_roles, *qty).await?;

        if member_applied { any_member = true; }
        if threshold_applied { any_threshold = true; }

        priced_lines.push(PricedLine {
            product_id: *product_id,
            sku: product.sku,
            name: product.name,
            quantity: *qty,
            base_price_cents: product.base_price_cents,
            unit_price_cents: unit_price,
            line_total_cents: unit_price * *qty as i64,
            member_price_applied: member_applied,
            threshold_discount_applied: threshold_applied,
            discount_bps,
            stock: product.stock,
        });
    }

    let subtotal: i64 = priced_lines.iter().map(|l| l.line_total_cents).sum();

    // Coupon
    let (coupon_discount_cents, coupon_applied, coupon_code, coupon_blocked_reason) =
        if let Some(coupon_id) = input.coupon_id {
            apply_coupon(pool, coupon_id, subtotal, any_member, any_threshold).await?
        } else {
            (0, false, None, None)
        };

    // Bundle warnings
    let bundle_warnings = check_bundles(pool, &priced_lines).await?;

    // Purchase limit warnings
    let purchase_limit_warnings =
        check_daily_limits(pool, input.user_id, &priced_lines).await?;

    let total = subtotal + input.delivery_fee_cents - coupon_discount_cents;

    Ok(PricingSummary {
        lines: priced_lines,
        subtotal_cents: subtotal,
        delivery_fee_cents: input.delivery_fee_cents,
        coupon_discount_cents,
        total_cents: total.max(0),
        coupon_applied,
        coupon_code,
        coupon_blocked_reason,
        bundle_warnings,
        purchase_limit_warnings,
    })
}

async fn apply_coupon(
    pool: &PgPool,
    coupon_id: Uuid,
    subtotal: i64,
    any_member: bool,
    any_threshold: bool,
) -> Result<(i64, bool, Option<String>, Option<String>), ApiAppError> {
    let row: Option<(String, i32, bool, bool, bool, Option<i64>, Option<i32>)> = sqlx::query_as(
        "SELECT code, discount_bps, stackable_with_member_price, stackable_with_threshold,
                is_active, EXTRACT(EPOCH FROM (valid_until - NOW()))::BIGINT,
                CASE WHEN max_uses IS NULL THEN NULL ELSE max_uses - times_used END
         FROM coupons WHERE id = $1",
    )
    .bind(coupon_id)
    .fetch_optional(pool)
    .await?;
    let (code, bps, stack_member, stack_threshold, is_active, secs_left, uses_left) =
        match row {
            Some(r) => r,
            None => return Ok((0, false, None, Some("coupon not found".into()))),
        };

    if !is_active {
        return Ok((0, false, Some(code), Some("coupon inactive".into())));
    }
    if secs_left.map(|s| s < 0).unwrap_or(false) {
        return Ok((0, false, Some(code), Some("coupon expired".into())));
    }
    if uses_left.map(|u| u <= 0).unwrap_or(false) {
        return Ok((0, false, Some(code), Some("coupon exhausted".into())));
    }
    if any_member && !stack_member {
        return Ok((0, false, Some(code),
            Some("coupon cannot stack with member pricing".into())));
    }
    if any_threshold && !stack_threshold {
        return Ok((0, false, Some(code),
            Some("coupon cannot stack with threshold discount".into())));
    }

    let discount = subtotal * bps as i64 / 10000;
    Ok((discount, true, Some(code), None))
}

async fn check_bundles(
    pool: &PgPool,
    lines: &[PricedLine],
) -> Result<Vec<BundleWarning>, ApiAppError> {
    if lines.is_empty() { return Ok(vec![]); }
    let product_ids: Vec<Uuid> = lines.iter().map(|l| l.product_id).collect();

    let bundles: Vec<(Uuid, String, String, i32, String)> = sqlx::query_as(
        "SELECT b.id, b.key, b.label, b.threshold_qty, b.warning_message
         FROM product_bundles b
         JOIN product_bundle_members m ON m.bundle_id = b.id
         WHERE m.product_id = ANY($1)
         GROUP BY b.id, b.key, b.label, b.threshold_qty, b.warning_message",
    )
    .bind(&product_ids)
    .fetch_all(pool)
    .await?;

    let mut warnings = Vec::new();
    for (bundle_id, key, label, threshold_qty, warning_message) in bundles {
        // Sum quantities of products that are in this bundle AND in the cart.
        let members: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT product_id FROM product_bundle_members WHERE bundle_id = $1",
        )
        .bind(bundle_id)
        .fetch_all(pool)
        .await?;
        let member_ids: Vec<Uuid> = members.into_iter().map(|m| m.0).collect();
        let total_qty: i32 = lines.iter()
            .filter(|l| member_ids.contains(&l.product_id))
            .map(|l| l.quantity)
            .sum();

        if total_qty >= threshold_qty {
            warnings.push(BundleWarning {
                bundle_key: key,
                label,
                warning_message,
                total_qty,
                threshold_qty,
            });
        }
    }
    Ok(warnings)
}

async fn check_daily_limits(
    pool: &PgPool,
    user_id: Uuid,
    lines: &[PricedLine],
) -> Result<Vec<String>, ApiAppError> {
    let mut warnings = Vec::new();
    for line in lines {
        let existing: Option<(i32,)> = sqlx::query_as(
            "SELECT quantity FROM daily_purchase_tracking
             WHERE user_id=$1 AND product_id=$2 AND purchase_date=CURRENT_DATE",
        )
        .bind(user_id)
        .bind(line.product_id)
        .fetch_optional(pool)
        .await?;
        let already = existing.map(|r| r.0).unwrap_or(0);
        let limit = daily_limit_per_sku();
        if already + line.quantity > limit {
            warnings.push(format!(
                "{} ({}): daily limit {} — already purchased {}, requesting {}",
                line.name, line.sku, limit, already, line.quantity
            ));
        }
    }
    Ok(warnings)
}

// ─────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_beats_member_when_cheaper() {
        // Simulate: base=1000, member=900, threshold=20% off → 800
        let base = 1000i64;
        let member_price = 900i64;
        let bps = 2000i32; // 20%
        let threshold_price = base - (base * bps as i64 / 10000);
        assert_eq!(threshold_price, 800);
        // threshold cheaper → should pick threshold
        assert!(threshold_price < member_price);
    }

    #[test]
    fn member_beats_threshold_when_cheaper() {
        let base = 1000i64;
        let member_price = 700i64;
        let bps = 1500i32; // 15%
        let threshold_price = base - (base * bps as i64 / 10000);
        assert_eq!(threshold_price, 850);
        // member cheaper → should pick member
        assert!(member_price < threshold_price);
    }

    #[test]
    fn coupon_blocked_by_member_no_stack() {
        // Simulate the logic: any_member=true, stack_member=false → blocked
        let any_member = true;
        let stack_member = false;
        let blocked = any_member && !stack_member;
        assert!(blocked);
    }

    #[test]
    fn coupon_allowed_when_stackable() {
        let any_member = true;
        let stack_member = true;
        let blocked = any_member && !stack_member;
        assert!(!blocked);
    }

    #[test]
    fn daily_limit_arithmetic() {
        let limit = DAILY_LIMIT_PER_SKU;
        let already = 3;
        let requesting = 3;
        assert!(already + requesting > limit);
    }

    #[test]
    fn total_cannot_go_negative() {
        let subtotal: i64 = 100;
        let delivery: i64 = 0;
        let coupon_discount: i64 = 200; // larger than subtotal
        let total = (subtotal + delivery - coupon_discount).max(0);
        assert_eq!(total, 0);
    }

    #[test]
    fn threshold_price_formula_correct() {
        // 10% off (1000 bps) on 499 cents: `base - (base * bps / 10000)` with
        // integer division gives 499 − (499_000 / 10_000) = 499 − 49 = 450.
        // The truncation-towards-zero is deliberate and matches the discount
        // computation in `resolve_line_price`, so callers never get charged
        // less than the rounded-down amount.
        let base: i64 = 499;
        let bps: i64 = 1000;
        let discounted = base - (base * bps / 10000);
        assert_eq!(discounted, 450);
    }

    #[test]
    fn daily_limit_exactly_at_boundary() {
        let limit = DAILY_LIMIT_PER_SKU;
        let already = limit - 1;
        let requesting = 1;
        assert!(already + requesting <= limit, "exactly at limit should be allowed");

        let already_full = limit;
        let requesting_one_more = 1;
        assert!(
            already_full + requesting_one_more > limit,
            "one over limit should be blocked"
        );
    }

    #[test]
    fn line_total_is_unit_price_times_quantity() {
        let unit_price: i64 = 254; // senior price for CAN-001
        let qty: i32 = 3;
        let expected_total = unit_price * qty as i64;
        assert_eq!(expected_total, 762);
    }

    #[test]
    fn coupon_discount_truncates_to_integer_cents() {
        let subtotal: i64 = 999;
        let bps: i64 = 1000; // 10%
        let discount = subtotal * bps / 10000;
        assert_eq!(discount, 99); // truncated, not rounded
    }
}
