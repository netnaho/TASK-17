use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use sqlx::PgPool;

/// Idempotent baseline + Phase 2 demo seed.
///
/// Seeds:
///  - canonical roles (Phase 1)
///  - permission catalog and role↔permission mapping (Phase 2)
///  - one institution + site + department for scope demos
///  - six demo users (one per role) with hashed passwords
///  - scope assignments for the department-level demo endpoint
///
/// Re-running on every container start is safe — every insert uses an
/// `ON CONFLICT DO NOTHING` clause keyed by a natural unique column.
pub async fn run(pool: &PgPool) -> Result<()> {
    tracing::info!("running baseline + auth seed");

    // ---- roles ----
    let roles: &[(&str, &str)] = &[
        ("admin", "Institution Admin"),
        ("medical", "Medical Staff"),
        ("dept_approver", "Department Approver"),
        ("finance_approver", "Finance Approver"),
        ("senior", "Senior"),
        ("family", "Family Member"),
    ];
    for (key, label) in roles {
        sqlx::query("INSERT INTO roles (key, label) VALUES ($1, $2) ON CONFLICT (key) DO NOTHING")
            .bind(key).bind(label).execute(pool).await?;
    }

    // ---- permissions ----
    let perms: &[(&str, &str, &str)] = &[
        ("*",                   "Wildcard (admin)",        "system"),
        ("scope:any",           "Bypass data scopes",      "system"),
        ("admin:users",         "Manage users",            "admin"),
        ("admin:blacklist",     "Manage IP blacklist",     "admin"),
        ("admin:settings",      "Manage settings",         "admin"),
        ("residents:read",      "Read residents",          "residents"),
        ("residents:write",     "Write residents",         "residents"),
        ("requisitions:read",   "Read requisitions",       "requisitions"),
        ("requisitions:write",  "Create requisitions",     "requisitions"),
        ("requisitions:approve","Approve requisitions",    "requisitions"),
        ("orders:read",         "Read orders",             "orders"),
        ("orders:write",        "Manage orders",           "orders"),
        ("inventory:read",      "Read inventory",          "inventory"),
        ("inventory:write",     "Manage inventory",        "inventory"),
        ("analytics:read",      "Read analytics",          "analytics"),
        ("family:read",         "Read family-portal data", "family"),
        ("moderation:review",   "Review moderation queue", "moderation"),
    ];
    for (key, label, cat) in perms {
        sqlx::query(
            "INSERT INTO permissions (key, label, category) VALUES ($1, $2, $3)
             ON CONFLICT (key) DO NOTHING",
        )
        .bind(key).bind(label).bind(cat).execute(pool).await?;
    }

    // ---- role -> permission mapping ----
    let role_perms: &[(&str, &[&str])] = &[
        ("admin", &["*", "scope:any", "admin:users", "admin:blacklist", "admin:settings"]),
        ("medical", &["residents:read", "residents:write", "requisitions:read", "requisitions:write"]),
        ("dept_approver", &["requisitions:read", "requisitions:approve", "residents:read"]),
        ("finance_approver", &["requisitions:read", "requisitions:approve", "orders:read", "orders:write", "analytics:read"]),
        ("senior", &["residents:read", "family:read"]),
        ("family", &["family:read"]),
    ];
    for (role_key, perm_keys) in role_perms {
        for pk in *perm_keys {
            sqlx::query(
                "INSERT INTO role_permissions (role_id, permission_id)
                 SELECT r.id, p.id FROM roles r, permissions p
                 WHERE r.key = $1 AND p.key = $2
                 ON CONFLICT DO NOTHING",
            )
            .bind(role_key).bind(pk).execute(pool).await?;
        }
    }

    // ---- one institution / site / department for demos ----
    sqlx::query(
        "INSERT INTO institutions (name, code) VALUES ('SilverOak Demo', 'demo')
         ON CONFLICT (code) DO NOTHING",
    ).execute(pool).await?;

    let inst: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM institutions WHERE code = 'demo'")
            .fetch_one(pool).await?;

    sqlx::query(
        "INSERT INTO sites (institution_id, name)
         SELECT $1, 'Main Campus' WHERE NOT EXISTS (
           SELECT 1 FROM sites WHERE institution_id = $1 AND name = 'Main Campus'
         )",
    ).bind(inst.0).execute(pool).await?;

    let site: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM sites WHERE institution_id = $1 AND name = 'Main Campus'")
            .bind(inst.0).fetch_one(pool).await?;

    sqlx::query(
        "INSERT INTO departments (site_id, name)
         SELECT $1, 'Wing A' WHERE NOT EXISTS (
           SELECT 1 FROM departments WHERE site_id = $1 AND name = 'Wing A'
         )",
    ).bind(site.0).execute(pool).await?;

    let dept: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM departments WHERE site_id = $1 AND name = 'Wing A'")
            .bind(site.0).fetch_one(pool).await?;

    // ---- demo users ----
    // Default password for ALL demo users (Phase 2 only): "ChangeMeNow!2025"
    // Documented in README. ≥12 characters, hashed with Argon2.
    let demo_password = "ChangeMeNow!2025";
    let hash = hash(demo_password)?;

    let demo_users: &[(&str, &str, &str)] = &[
        ("admin@silveroak.local",            "Avery Admin",      "admin"),
        ("medical@silveroak.local",          "Morgan Medical",   "medical"),
        ("approver@silveroak.local",         "Devon Department", "dept_approver"),
        ("finance@silveroak.local",          "Frankie Finance",  "finance_approver"),
        ("senior@silveroak.local",           "Sage Senior",      "senior"),
        ("family@silveroak.local",           "Riley Relative",   "family"),
        // Dedicated lockout-test account — never used for real test flows
        ("lockout_test@silveroak.local",     "Lockout Tester",   "medical"),
    ];

    for (email, name, role_key) in demo_users {
        sqlx::query(
            "INSERT INTO users (email, display_name, password_hash)
             VALUES ($1, $2, $3) ON CONFLICT (email) DO NOTHING",
        )
        .bind(email).bind(name).bind(&hash).execute(pool).await?;

        sqlx::query(
            "INSERT INTO user_role_assignments (user_id, role_id, site_id)
             SELECT u.id, r.id, $3 FROM users u, roles r
             WHERE u.email = $1 AND r.key = $2
             ON CONFLICT DO NOTHING",
        )
        .bind(email).bind(role_key).bind(site.0).execute(pool).await?;
    }

    // Department-scoped users get the demo department in their scopes.
    // finance@silveroak.local needs dept scope so load_detail works after approving.
    for email in ["medical@silveroak.local", "approver@silveroak.local", "finance@silveroak.local"] {
        sqlx::query(
            "INSERT INTO user_scopes (user_id, scope_kind, scope_ref)
             SELECT u.id, 'department', $2 FROM users u WHERE u.email = $1
             ON CONFLICT DO NOTHING",
        )
        .bind(email).bind(dept.0.to_string()).execute(pool).await?;
    }

    // Family user gets a family_group scope (demo).
    sqlx::query(
        "INSERT INTO user_scopes (user_id, scope_kind, scope_ref)
         SELECT u.id, 'family_group', 'demo-family' FROM users u
         WHERE u.email = 'family@silveroak.local' ON CONFLICT DO NOTHING",
    ).execute(pool).await?;

    // ---- Phase 3: inventory + approval engine ----
    seed_inventory_and_workflow(pool, site.0).await?;

    // ---- Phase 4: consumable ordering ----
    seed_orders(pool).await?;

    // ---- Phase 6: moderation policies, anomaly rules, demo family group ----
    seed_phase6(pool).await?;

    // ---- Sample data for reviewer inspection ----
    seed_sample_data(pool, site.0, inst.0).await?;

    tracing::info!("seed complete");
    Ok(())
}

async fn seed_inventory_and_workflow(pool: &PgPool, site_id: uuid::Uuid) -> Result<()> {
    // Categories
    let categories: &[(&str, &str, bool)] = &[
        ("ppe",          "Personal Protective Equipment", false),
        ("medical",      "Medical Supplies",              false),
        ("controlled",   "Controlled Substances",         true),
        ("housekeeping", "Housekeeping",                  false),
    ];
    for (key, label, controlled) in categories {
        sqlx::query(
            "INSERT INTO inventory_categories (key, label, controlled) VALUES ($1,$2,$3)
             ON CONFLICT (key) DO NOTHING",
        ).bind(key).bind(label).bind(controlled).execute(pool).await?;
    }

    // Items: (sku, name, category_key, unit, unit_price_cents, on_hand)
    let items: &[(&str, &str, &str, &str, i64, i32)] = &[
        ("PPE-001", "Nitrile Gloves (box of 100)", "ppe",          "box",  1295,  120),
        ("PPE-002", "Surgical Masks (box of 50)",  "ppe",          "box",   899,  200),
        ("MED-001", "Adult Briefs (case)",         "medical",      "case", 3450,   80),
        ("MED-002", "Wound Dressing 4x4 (pack)",   "medical",      "pack",  599,  150),
        ("CTL-001", "Acetaminophen 500mg (bottle)","controlled",   "btl",   799,   60),
        ("CTL-002", "Lorazepam 1mg (bottle)",      "controlled",   "btl",  4250,   25),
        ("HSK-001", "Disinfectant Wipes (canister)","housekeeping","can",   549,  300),
    ];
    for (sku, name, cat, unit, price, on_hand) in items {
        sqlx::query(
            "INSERT INTO inventory_items (sku, name, category_id, unit, unit_price_cents)
             SELECT $1, $2, c.id, $4, $5 FROM inventory_categories c WHERE c.key = $3
             ON CONFLICT (sku) DO NOTHING",
        ).bind(sku).bind(name).bind(cat).bind(unit).bind(price).execute(pool).await?;

        sqlx::query(
            "INSERT INTO inventory_stock_by_site (item_id, site_id, on_hand)
             SELECT i.id, $2, $3 FROM inventory_items i WHERE i.sku = $1
             ON CONFLICT (item_id, site_id) DO NOTHING",
        ).bind(sku).bind(site_id).bind(on_hand).execute(pool).await?;
    }

    // Approval workflow: requisition_default
    sqlx::query(
        "INSERT INTO approval_workflows (key, label) VALUES ('requisition_default', 'Default requisition workflow')
         ON CONFLICT (key) DO NOTHING",
    ).execute(pool).await?;

    let wf: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM approval_workflows WHERE key='requisition_default'")
            .fetch_one(pool).await?;

    // Node 1: department approver, always.
    sqlx::query(
        "INSERT INTO approval_nodes (workflow_id, sequence, label, required_role)
         VALUES ($1, 1, 'Department Approver', 'dept_approver')
         ON CONFLICT (workflow_id, sequence) DO NOTHING",
    ).bind(wf.0).execute(pool).await?;

    // Node 2: finance approver, when amount > $2,500.00 OR controlled.
    sqlx::query(
        "INSERT INTO approval_nodes (workflow_id, sequence, label, required_role)
         VALUES ($1, 2, 'Finance Approver', 'finance_approver')
         ON CONFLICT (workflow_id, sequence) DO NOTHING",
    ).bind(wf.0).execute(pool).await?;

    let node1: (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM approval_nodes WHERE workflow_id=$1 AND sequence=1",
    ).bind(wf.0).fetch_one(pool).await?;
    let node2: (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM approval_nodes WHERE workflow_id=$1 AND sequence=2",
    ).bind(wf.0).fetch_one(pool).await?;

    // Conditions are seeded only when the node has none yet, so re-running
    // the seed never duplicates them.
    let n1_has: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM approval_rule_conditions WHERE node_id=$1")
        .bind(node1.0).fetch_one(pool).await?;
    if n1_has.0 == 0 {
        sqlx::query("INSERT INTO approval_rule_conditions (node_id, condition_kind) VALUES ($1, 'always')")
            .bind(node1.0).execute(pool).await?;
    }
    let n2_has: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM approval_rule_conditions WHERE node_id=$1")
        .bind(node2.0).fetch_one(pool).await?;
    if n2_has.0 == 0 {
        sqlx::query("INSERT INTO approval_rule_conditions (node_id, condition_kind, threshold_cents) VALUES ($1, 'amount_gt_cents', 250000)")
            .bind(node2.0).execute(pool).await?;
        sqlx::query("INSERT INTO approval_rule_conditions (node_id, condition_kind) VALUES ($1, 'contains_controlled')")
            .bind(node2.0).execute(pool).await?;
    }

    Ok(())
}

async fn seed_orders(pool: &sqlx::PgPool) -> Result<()> {
    // Delivery methods
    let methods: &[(&str, &str, i64)] = &[
        ("pickup",    "Facility Pickup",     0),
        ("courier",   "Internal Courier",  750),
        ("scheduled", "Scheduled Delivery",1200),
    ];
    for (key, label, fee) in methods {
        sqlx::query(
            "INSERT INTO delivery_methods (key, label, fee_cents) VALUES ($1,$2,$3)
             ON CONFLICT (key) DO NOTHING",
        ).bind(key).bind(label).bind(fee).execute(pool).await?;
    }

    // Consumable products: (sku, name, store_key, base_price_cents, stock)
    let products: &[(&str, &str, &str, i64, i32)] = &[
        ("CAN-001", "Orange Juice 250ml",    "canteen",  299,  50),
        ("CAN-002", "Herbal Tea Bag x10",    "canteen",  499,  80),
        ("CAN-003", "Oatmeal Biscuits 200g", "canteen",  350,  60),
        ("PHR-001", "Vitamin C 500mg x30",   "pharmacy", 895,  40),
        ("PHR-002", "Throat Lozenges x16",   "pharmacy", 450,  35),
        ("GFT-001", "Greeting Card",         "gift",     299, 100),
        ("GFT-002", "Small Flower Bouquet",  "gift",    1200,  20),
    ];
    for (sku, name, store, price, stock) in products {
        sqlx::query(
            "INSERT INTO product_catalog (sku, name, store_key, base_price_cents, stock)
             VALUES ($1,$2,$3,$4,$5) ON CONFLICT (sku) DO NOTHING",
        ).bind(sku).bind(name).bind(store).bind(price).bind(stock).execute(pool).await?;
    }

    // Member pricing: seniors get 15% off canteen items (expressed as price)
    // CAN-001 base 299 → senior 254; CAN-002 base 499 → senior 424; CAN-003 base 350 → senior 297
    let senior_prices: &[(&str, i64)] = &[
        ("CAN-001", 254),
        ("CAN-002", 424),
        ("CAN-003", 297),
    ];
    for (sku, price) in senior_prices {
        sqlx::query(
            "INSERT INTO product_pricing (product_id, role_key, price_cents)
             SELECT p.id, 'senior', $2 FROM product_catalog p WHERE p.sku=$1
             ON CONFLICT (product_id, role_key) DO NOTHING",
        ).bind(sku).bind(price).execute(pool).await?;
    }

    // Threshold discounts: buy 3+ tea bags → 10% off
    sqlx::query(
        "INSERT INTO product_threshold_discounts (product_id, min_quantity, discount_bps)
         SELECT p.id, 3, 1000 FROM product_catalog p WHERE p.sku='CAN-002'
         ON CONFLICT (product_id, min_quantity) DO NOTHING",
    ).execute(pool).await?;

    // Bundle warning: 5+ tea + biscuit combo
    sqlx::query(
        "INSERT INTO product_bundles (key, label, threshold_qty, warning_message)
         VALUES ('canteen_snack_bundle','Canteen Snack Bundle',5,
                 'You have ordered 5 or more snack/beverage items — consider the bulk catering option.')
         ON CONFLICT (key) DO NOTHING",
    ).execute(pool).await?;
    for sku in ["CAN-002", "CAN-003"] {
        sqlx::query(
            "INSERT INTO product_bundle_members (bundle_id, product_id)
             SELECT b.id, p.id FROM product_bundles b, product_catalog p
             WHERE b.key='canteen_snack_bundle' AND p.sku=$1
             ON CONFLICT DO NOTHING",
        ).bind(sku).execute(pool).await?;
    }

    // Coupons
    // WELCOME10: 10% off, cannot stack with member or threshold
    sqlx::query(
        "INSERT INTO coupons (code, discount_bps, stackable_with_member_price, stackable_with_threshold)
         VALUES ('WELCOME10', 1000, false, false)
         ON CONFLICT (code) DO NOTHING",
    ).execute(pool).await?;

    // LOYAL5: 5% off, stackable with member price only
    sqlx::query(
        "INSERT INTO coupons (code, discount_bps, stackable_with_member_price, stackable_with_threshold)
         VALUES ('LOYAL5', 500, true, false)
         ON CONFLICT (code) DO NOTHING",
    ).execute(pool).await?;

    // Update role_permissions to grant orders:read/write to medical + senior + family
    let order_perms: &[(&str, &[&str])] = &[
        ("medical",        &["orders:read", "orders:write"]),
        ("senior",         &["orders:read", "orders:write"]),
        ("family",         &["orders:read", "orders:write"]),
        ("dept_approver",  &["orders:read"]),
    ];
    for (role_key, perm_keys) in order_perms {
        for pk in *perm_keys {
            sqlx::query(
                "INSERT INTO role_permissions (role_id, permission_id)
                 SELECT r.id, p.id FROM roles r, permissions p
                 WHERE r.key=$1 AND p.key=$2
                 ON CONFLICT DO NOTHING",
            ).bind(role_key).bind(pk).execute(pool).await?;
        }
    }

    Ok(())
}

fn hash(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    Ok(argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash error: {e}"))?
        .to_string())
}

async fn seed_phase6(pool: &PgPool) -> Result<()> {
    tracing::info!("seeding phase 6: moderation policies, anomaly rules, demo family group");

    // ---- keyword policies ----
    let policies: &[(&str, &str)] = &[
        ("violence", "high"),
        ("abuse", "high"),
        ("illegal", "high"),
        ("threat", "medium"),
        ("urgent help", "medium"),
        ("pain", "low"),
        ("discomfort", "low"),
    ];
    for (keyword, severity) in policies {
        sqlx::query(
            "INSERT INTO keyword_policies (keyword, severity, is_active)
             VALUES ($1, $2, true)
             ON CONFLICT (keyword) DO NOTHING",
        )
        .bind(keyword)
        .bind(severity)
        .execute(pool)
        .await?;
    }

    // ---- anomaly rules ----
    let rules: &[(&str, &str, &str, i32, i32, &str)] = &[
        (
            "auth_failure_burst",
            "Authentication Failure Burst",
            "More than 5 auth failures from the same IP within 1 hour",
            5,
            3600,
            "high",
        ),
        (
            "privilege_escalation_attempt",
            "Privilege Escalation Attempt",
            "User triggered 3+ permission denials within 1 hour",
            3,
            3600,
            "high",
        ),
        (
            "large_upload_near_limit",
            "Large File Upload",
            "File upload exceeding 8MB near the 10MB limit",
            1,
            60,
            "low",
        ),
    ];
    for (rule_key, label, description, threshold_count, window_seconds, severity) in rules {
        sqlx::query(
            "INSERT INTO anomaly_rules
                 (rule_key, label, description, threshold_count, window_seconds, severity)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (rule_key) DO NOTHING",
        )
        .bind(rule_key)
        .bind(label)
        .bind(description)
        .bind(threshold_count)
        .bind(window_seconds)
        .bind(severity)
        .execute(pool)
        .await?;
    }

    // ---- demo family group (only if none exist) ----
    let (family_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM family_groups")
            .fetch_one(pool)
            .await?;

    if family_count == 0 {
        // Get first institution
        let inst: Option<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM institutions ORDER BY name LIMIT 1")
                .fetch_optional(pool)
                .await?;

        if let Some((institution_id,)) = inst {
            let (group_id,): (uuid::Uuid,) = sqlx::query_as(
                "INSERT INTO family_groups (institution_id, label)
                 VALUES ($1, 'Demo Family')
                 RETURNING id",
            )
            .bind(institution_id)
            .fetch_one(pool)
            .await?;

            // Consent records for all 3 categories
            let categories = ["supply_usage", "wellness_summary", "activity_log"];
            for category in &categories {
                sqlx::query(
                    "INSERT INTO consent_records
                         (family_group_id, data_category, consented, consented_at)
                     VALUES ($1, $2, true, NOW())
                     ON CONFLICT (family_group_id, data_category) DO NOTHING",
                )
                .bind(group_id)
                .bind(category)
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(())
}

/// Seed realistic sample data for reviewer inspection.
/// Only runs when no requisitions exist — fully idempotent.
async fn seed_sample_data(pool: &PgPool, site_id: uuid::Uuid, inst_id: uuid::Uuid) -> Result<()> {
    let (req_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM requisitions")
        .fetch_one(pool)
        .await?;
    if req_count > 0 {
        return Ok(());
    }
    tracing::info!("seeding sample requisitions, moderation, and anomaly events");

    let dept_id: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM departments WHERE site_id = $1 LIMIT 1")
            .bind(site_id)
            .fetch_one(pool)
            .await?;

    let medical_id: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email='medical@silveroak.local'",
    )
    .fetch_optional(pool)
    .await?;
    let approver_id: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email='approver@silveroak.local'",
    )
    .fetch_optional(pool)
    .await?;
    let wf: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM approval_workflows WHERE key='requisition_default'",
    )
    .fetch_optional(pool)
    .await?;

    let (Some((medical_id,)), Some((approver_id,)), Some((wf_id,))) =
        (medical_id, approver_id, wf)
    else {
        tracing::warn!("sample data skipped: prerequisite rows not found");
        return Ok(());
    };

    let ppe_item: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM inventory_items WHERE sku='PPE-001'")
            .fetch_optional(pool)
            .await?;
    let ctl_item: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM inventory_items WHERE sku='CTL-002'")
            .fetch_optional(pool)
            .await?;
    let med_item: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM inventory_items WHERE sku='MED-001'")
            .fetch_optional(pool)
            .await?;

    // ── Requisition 1: DRAFT ──────────────────────────────────────────────
    let (r1_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO requisitions (ref_code, site_id, department_id, requester_id, status, needed_by, justification)
         VALUES ('REQ-SEED-001', $1, $2, $3, 'draft', CURRENT_DATE + 7,
                 'Monthly PPE restock for Wing A nursing station')
         RETURNING id",
    )
    .bind(site_id)
    .bind(dept_id.0)
    .bind(medical_id)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO requisition_audit (requisition_id, actor_user_id, event_kind) VALUES ($1,$2,'created')",
    )
    .bind(r1_id)
    .bind(medical_id)
    .execute(pool)
    .await?;
    if let Some((item_id,)) = ppe_item {
        sqlx::query(
            "INSERT INTO requisition_lines (requisition_id, item_id, quantity, unit_price_cents, line_total_cents)
             SELECT $1, $2, 5, unit_price_cents, unit_price_cents * 5
             FROM inventory_items WHERE id = $2",
        )
        .bind(r1_id)
        .bind(item_id)
        .execute(pool)
        .await?;
    }

    // ── Requisition 2: PENDING_APPROVAL (controlled) ──────────────────────
    let (r2_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO requisitions
             (ref_code, site_id, department_id, requester_id, status, needed_by, justification,
              contains_controlled)
         VALUES ('REQ-SEED-002', $1, $2, $3, 'pending_approval', CURRENT_DATE + 3,
                 'Controlled substance replenishment — Lorazepam stock below par level',
                 true)
         RETURNING id",
    )
    .bind(site_id)
    .bind(dept_id.0)
    .bind(medical_id)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        "INSERT INTO requisition_audit (requisition_id, actor_user_id, event_kind) VALUES ($1,$2,'submitted')",
    )
    .bind(r2_id)
    .bind(medical_id)
    .execute(pool)
    .await?;
    if let Some((item_id,)) = ctl_item {
        sqlx::query(
            "INSERT INTO requisition_lines (requisition_id, item_id, quantity, unit_price_cents, line_total_cents)
             SELECT $1, $2, 10, unit_price_cents, unit_price_cents * 10
             FROM inventory_items WHERE id = $2",
        )
        .bind(r2_id)
        .bind(item_id)
        .execute(pool)
        .await?;
        sqlx::query(
            "UPDATE requisitions SET total_amount_cents =
             (SELECT COALESCE(SUM(i.unit_price_cents * l.quantity),0)
              FROM requisition_lines l JOIN inventory_items i ON i.id=l.item_id
              WHERE l.requisition_id=$1)
             WHERE id=$1",
        )
        .bind(r2_id)
        .execute(pool)
        .await?;
    }
    // Create approval instance + pending step for the approver inbox
    let node1: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM approval_nodes WHERE workflow_id=$1 AND sequence=1",
    )
    .bind(wf_id)
    .fetch_optional(pool)
    .await?;
    if let Some((n1_id,)) = node1 {
        let (ai_id,): (uuid::Uuid,) = sqlx::query_as(
            "INSERT INTO requisition_approval_instances (requisition_id, workflow_id, current_step)
             VALUES ($1, $2, 1) RETURNING id",
        )
        .bind(r2_id)
        .bind(wf_id)
        .fetch_one(pool)
        .await?;
        sqlx::query(
            "INSERT INTO requisition_approval_steps (instance_id, sequence, node_id, required_role)
             VALUES ($1, 1, $2, 'dept_approver')",
        )
        .bind(ai_id)
        .bind(n1_id)
        .execute(pool)
        .await?;
    }

    // ── Requisition 3: REJECTED ───────────────────────────────────────────
    let (r3_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO requisitions (ref_code, site_id, department_id, requester_id, status, needed_by, justification)
         VALUES ('REQ-SEED-003', $1, $2, $3, 'rejected', CURRENT_DATE - 5,
                 'Additional wound dressings for increased occupancy')
         RETURNING id",
    )
    .bind(site_id)
    .bind(dept_id.0)
    .bind(medical_id)
    .fetch_one(pool)
    .await?;
    if let Some((item_id,)) = med_item {
        sqlx::query(
            "INSERT INTO requisition_lines (requisition_id, item_id, quantity, unit_price_cents, line_total_cents)
             SELECT $1, $2, 20, unit_price_cents, unit_price_cents * 20
             FROM inventory_items WHERE id = $2",
        )
        .bind(r3_id)
        .bind(item_id)
        .execute(pool)
        .await?;
    }
    sqlx::query(
        "INSERT INTO requisition_audit (requisition_id, actor_user_id, event_kind, comment)
         VALUES ($1,$2,'rejected',
                 'Budget exceeded for current period. Please resubmit next quarter.')",
    )
    .bind(r3_id)
    .bind(approver_id)
    .execute(pool)
    .await?;

    // ── Sample moderation queue item ──────────────────────────────────────
    let _ = sqlx::query(
        "INSERT INTO moderation_queue
             (content_type, content_snippet, matched_keywords, severity, status)
         VALUES ('wellness_note',
                 'Resident reported pain during morning exercises — physiotherapy recommended',
                 ARRAY['pain'], 'low', 'pending')",
    )
    .execute(pool)
    .await;

    // ── Sample anomaly event ──────────────────────────────────────────────
    let _ = sqlx::query(
        "INSERT INTO anomaly_events
             (rule_key, severity, subject_type, subject_value, detail, acknowledged)
         VALUES ('auth_failure_burst', 'high', 'ip', '192.168.1.100',
                 '{\"failure_count\": 6, \"window_seconds\": 3600}'::jsonb, false)",
    )
    .execute(pool)
    .await;

    // ── Sample wellness activity ──────────────────────────────────────────
    if let Some((student_id,)) = sqlx::query_as::<_, (uuid::Uuid,)>("SELECT id FROM students LIMIT 1")
        .fetch_optional(pool)
        .await?
    {
        let _ = sqlx::query(
            "INSERT INTO wellness_activities
                 (institution_id, student_id, activity_type, duration_minutes,
                  notes_encrypted, activity_date)
             VALUES ($1, $2, 'exercise', 30,
                     'Completed morning walk without assistance',
                     CURRENT_DATE - 1)",
        )
        .bind(inst_id)
        .bind(student_id)
        .execute(pool)
        .await;
    }

    // ── Seed analytics rows so dashboard shows non-zero live counts ───────
    let _ = sqlx::query(
        "INSERT INTO analytics_daily
             (institution_id, stat_date, stat_key, stat_value)
         VALUES
             ($1, CURRENT_DATE - 1, 'requisition_count', 3),
             ($1, CURRENT_DATE - 1, 'order_count', 1),
             ($1, CURRENT_DATE - 1, 'supply_spend_cents', 42500),
             ($1, CURRENT_DATE - 1, 'approved_requisitions', 0)
         ON CONFLICT (institution_id, stat_date, stat_key) DO NOTHING",
    )
    .bind(inst_id)
    .execute(pool)
    .await;

    tracing::info!("sample data seed complete");
    Ok(())
}
