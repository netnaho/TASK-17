use actix_web::{get, HttpResponse, Responder};
use shared::HealthStatus;

#[get("/health")]
pub async fn get_health() -> impl Responder {
    HttpResponse::Ok().json(HealthStatus {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
