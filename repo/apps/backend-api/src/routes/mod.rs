use actix_web::web;

use crate::security::{middleware::SignedAuth, signing_mode::SigningMode};

pub mod admin;
pub mod analytics;
pub mod anomaly;
pub mod auth;
pub mod family;
pub mod health;
pub mod master_data;
pub mod moderation;
pub mod orders;
pub mod requisitions;

pub fn configure(cfg: &mut web::ServiceConfig, mode: SigningMode) {
    cfg.service(
        web::scope("/api/v1")
            // ── truly public (no auth required) ──────────────────────────
            .service(health::get_health)
            .service(auth::login)
            .service(auth::request_recovery)
            // ── session-cookie-authenticated (NOT behind SignedAuth) ──────
            // logout and current_session validate the session cookie
            // internally.  They are intentionally outside the SignedAuth
            // scope because the short-lived API token (15 min TTL) may have
            // expired by the time the client calls them — notably, a client
            // cannot log out using a token it no longer holds.
            .service(auth::logout)
            .service(auth::current_session)
            // ---- authenticated, signed scope ----
            .service(
                web::scope("")
                    .wrap(SignedAuth { mode })
                    .service(admin::me)
                    .service(admin::list_blacklist)
                    .service(admin::add_blacklist)
                    .service(admin::remove_blacklist)
                    .service(admin::grant_role)
                    .service(admin::list_department_residents)
                    // requisitions / approvals / inventory catalog
                    .service(requisitions::catalog)
                    .service(requisitions::create)
                    .service(requisitions::submit)
                    .service(requisitions::withdraw)
                    .service(requisitions::approve)
                    .service(requisitions::reject)
                    .service(requisitions::send_back)
                    .service(requisitions::list_mine)
                    .service(requisitions::inbox)
                    .service(requisitions::audit_timeline)
                    .service(requisitions::get_one)
                    .service(requisitions::issue_record)
                    // orders / cart / checkout
                    .service(orders::products)
                    .service(orders::delivery_methods)
                    .service(orders::get_cart)
                    .service(orders::set_line)
                    .service(orders::apply_coupon)
                    .service(orders::remove_coupon)
                    .service(orders::set_delivery)
                    .service(orders::verify)
                    .service(orders::confirm)
                    .service(orders::list_mine)
                    .service(orders::get_order)
                    // master data / admin CRUD
                    .service(master_data::list_institutions)
                    .service(master_data::list_semesters)
                    .service(master_data::get_semester)
                    .service(master_data::create_semester)
                    .service(master_data::update_semester)
                    .service(master_data::delete_semester)
                    .service(master_data::list_classes)
                    .service(master_data::get_class)
                    .service(master_data::create_class)
                    .service(master_data::update_class)
                    .service(master_data::delete_class)
                    .service(master_data::list_courses)
                    .service(master_data::get_course)
                    .service(master_data::create_course)
                    .service(master_data::update_course)
                    .service(master_data::delete_course)
                    .service(master_data::list_students)
                    .service(master_data::get_student)
                    .service(master_data::create_student)
                    .service(master_data::update_student)
                    .service(master_data::delete_student)
                    .service(master_data::list_departments)
                    .service(master_data::get_department)
                    .service(master_data::create_department)
                    .service(master_data::update_department)
                    .service(master_data::delete_department)
                    .service(master_data::run_import)
                    .service(master_data::get_import_job)
                    .service(master_data::export_entity)
                    // family portal
                    .service(family::list_my_groups)
                    .service(family::create_group)
                    .service(family::get_group)
                    .service(family::add_member)
                    .service(family::add_resident)
                    .service(family::set_consent)
                    .service(family::get_supply_summary)
                    .service(family::get_wellness_summary)
                    .service(family::get_activity_log)
                    .service(family::log_wellness)
                    // analytics
                    .service(analytics::dashboard)
                    .service(analytics::daily_stats)
                    .service(analytics::weekly_stats)
                    .service(analytics::monthly_stats)
                    // moderation
                    .service(moderation::list_queue)
                    .service(moderation::review_item)
                    .service(moderation::list_policies)
                    .service(moderation::create_policy)
                    .service(moderation::delete_policy)
                    // anomaly
                    .service(anomaly::list_events)
                    .service(anomaly::acknowledge_event)
                    .service(anomaly::list_rules),
            ),
    );
}
