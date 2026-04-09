-- Phase 4: consumable ordering
-- Delivery methods ---------------------------------------------------------
CREATE TABLE delivery_methods (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key         TEXT NOT NULL UNIQUE,
    label       TEXT NOT NULL,
    fee_cents   BIGINT NOT NULL DEFAULT 0,
    is_active   BOOLEAN NOT NULL DEFAULT true
);

-- Consumable product catalog -----------------------------------------------
CREATE TABLE product_catalog (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sku              TEXT NOT NULL UNIQUE,
    name             TEXT NOT NULL,
    store_key        TEXT NOT NULL,  -- 'canteen' | 'pharmacy' | 'gift'
    base_price_cents BIGINT NOT NULL CHECK (base_price_cents >= 0),
    stock            INT NOT NULL DEFAULT 0 CHECK (stock >= 0),
    is_active        BOOLEAN NOT NULL DEFAULT true,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Member pricing (role-specific price) -------------------------------------
CREATE TABLE product_pricing (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id  UUID NOT NULL REFERENCES product_catalog(id),
    role_key    TEXT NOT NULL,
    price_cents BIGINT NOT NULL CHECK (price_cents >= 0),
    UNIQUE (product_id, role_key)
);

-- Threshold / bulk discounts -----------------------------------------------
CREATE TABLE product_threshold_discounts (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id    UUID NOT NULL REFERENCES product_catalog(id),
    min_quantity  INT NOT NULL CHECK (min_quantity > 0),
    discount_bps  INT NOT NULL CHECK (discount_bps > 0 AND discount_bps <= 10000),
    UNIQUE (product_id, min_quantity)
);

-- Bundle thresholds (cross-product qty warnings) ---------------------------
CREATE TABLE product_bundles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key             TEXT NOT NULL UNIQUE,
    label           TEXT NOT NULL,
    threshold_qty   INT NOT NULL CHECK (threshold_qty > 0),
    warning_message TEXT NOT NULL
);

CREATE TABLE product_bundle_members (
    bundle_id  UUID NOT NULL REFERENCES product_bundles(id),
    product_id UUID NOT NULL REFERENCES product_catalog(id),
    PRIMARY KEY (bundle_id, product_id)
);

-- Coupons ------------------------------------------------------------------
CREATE TABLE coupons (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code                        TEXT NOT NULL UNIQUE,
    discount_bps                INT NOT NULL CHECK (discount_bps > 0 AND discount_bps <= 10000),
    valid_from                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    valid_until                 TIMESTAMPTZ,
    max_uses                    INT,
    times_used                  INT NOT NULL DEFAULT 0,
    stackable_with_member_price BOOLEAN NOT NULL DEFAULT false,
    stackable_with_threshold    BOOLEAN NOT NULL DEFAULT false,
    is_active                   BOOLEAN NOT NULL DEFAULT true
);

-- Shopping carts -----------------------------------------------------------
CREATE TABLE carts (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id            UUID NOT NULL REFERENCES users(id),
    coupon_id          UUID REFERENCES coupons(id),
    delivery_method_id UUID REFERENCES delivery_methods(id),
    is_active          BOOLEAN NOT NULL DEFAULT true,
    store_key          TEXT,           -- NULL = empty/mixed; populated when first item added
    has_cross_store    BOOLEAN NOT NULL DEFAULT false,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE cart_lines (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cart_id    UUID NOT NULL REFERENCES carts(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES product_catalog(id),
    quantity   INT NOT NULL CHECK (quantity > 0),
    UNIQUE (cart_id, product_id)
);

-- Orders -------------------------------------------------------------------
CREATE TABLE orders (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ref_code            TEXT NOT NULL UNIQUE,
    user_id             UUID NOT NULL REFERENCES users(id),
    delivery_method_id  UUID NOT NULL REFERENCES delivery_methods(id),
    coupon_id           UUID REFERENCES coupons(id),
    status              TEXT NOT NULL DEFAULT 'confirmed'
                            CHECK (status IN ('confirmed','cancelled')),
    subtotal_cents      BIGINT NOT NULL,
    delivery_fee_cents  BIGINT NOT NULL,
    discount_cents      BIGINT NOT NULL DEFAULT 0,
    total_cents         BIGINT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    cancelled_at        TIMESTAMPTZ
);

CREATE TABLE order_lines (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id                    UUID NOT NULL REFERENCES orders(id),
    product_id                  UUID NOT NULL REFERENCES product_catalog(id),
    quantity                    INT NOT NULL,
    unit_price_cents            BIGINT NOT NULL,
    line_total_cents            BIGINT NOT NULL,
    member_price_applied        BOOLEAN NOT NULL DEFAULT false,
    threshold_discount_applied  BOOLEAN NOT NULL DEFAULT false,
    discount_bps                INT NOT NULL DEFAULT 0
);

CREATE TABLE order_adjustments (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id     UUID NOT NULL REFERENCES orders(id),
    kind         TEXT NOT NULL,    -- 'coupon' | 'delivery_fee'
    label        TEXT NOT NULL,
    amount_cents BIGINT NOT NULL   -- negative = discount, positive = fee
);

-- Daily purchase tracking (limit: 5 units/SKU/user/day) -------------------
CREATE TABLE daily_purchase_tracking (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users(id),
    product_id    UUID NOT NULL REFERENCES product_catalog(id),
    purchase_date DATE NOT NULL DEFAULT CURRENT_DATE,
    quantity      INT NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    UNIQUE (user_id, product_id, purchase_date)
);

-- Verification snapshots (server-side recheck before confirm) -------------
CREATE TABLE order_verification_snapshots (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cart_id       UUID NOT NULL REFERENCES carts(id),
    user_id       UUID NOT NULL REFERENCES users(id),
    snapshot_json JSONB NOT NULL,
    verified_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at    TIMESTAMPTZ NOT NULL
);

-- Indexes ------------------------------------------------------------------
CREATE INDEX idx_cart_lines_cart ON cart_lines(cart_id);
CREATE INDEX idx_order_lines_order ON order_lines(order_id);
CREATE INDEX idx_daily_track_user_date ON daily_purchase_tracking(user_id, purchase_date);
CREATE INDEX idx_snapshots_cart ON order_verification_snapshots(cart_id, expires_at);
