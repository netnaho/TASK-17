use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::orders::service;
use crate::security::rbac::Principal;

// GET /api/v1/orders/products
#[get("/orders/products")]
pub async fn products(
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let items = service::list_products(&pool).await?;
    Ok(HttpResponse::Ok().json(items))
}

// GET /api/v1/orders/delivery-methods
#[get("/orders/delivery-methods")]
pub async fn delivery_methods(
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let methods = service::list_delivery_methods(&pool).await?;
    Ok(HttpResponse::Ok().json(methods))
}

// GET /api/v1/orders/cart
#[get("/orders/cart")]
pub async fn get_cart(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let cart = service::get_cart(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(cart))
}

// POST /api/v1/orders/cart/lines
#[post("/orders/cart/lines")]
pub async fn set_line(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    body: web::Json<service::SetLineInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let cart = service::set_line(&pool, &p, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(cart))
}

// POST /api/v1/orders/cart/coupon
#[post("/orders/cart/coupon")]
pub async fn apply_coupon(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    body: web::Json<service::ApplyCouponInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let cart = service::apply_coupon(&pool, &p, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(cart))
}

// POST /api/v1/orders/cart/coupon-remove
#[post("/orders/cart/coupon-remove")]
pub async fn remove_coupon(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let cart = service::remove_coupon(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(cart))
}

// POST /api/v1/orders/cart/delivery
#[post("/orders/cart/delivery")]
pub async fn set_delivery(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    body: web::Json<service::SetDeliveryInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let cart = service::set_delivery(&pool, &p, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(cart))
}

// POST /api/v1/orders/verify
#[post("/orders/verify")]
pub async fn verify(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let summary = service::verify_cart(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(summary))
}

// POST /api/v1/orders/confirm
#[post("/orders/confirm")]
pub async fn confirm(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let order = service::confirm_order(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(order))
}

// GET /api/v1/orders/mine
#[get("/orders/mine")]
pub async fn list_mine(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let orders = service::list_my_orders(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(orders))
}

// GET /api/v1/orders/{id}
#[get("/orders/{id}")]
pub async fn get_order(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = req.extensions().get::<Principal>().cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))?;
    let order = service::load_order(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(order))
}
