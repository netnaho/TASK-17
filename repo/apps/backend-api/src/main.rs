mod analytics;
mod anomaly;
mod config;
mod error;
mod family;
mod master_data;
mod moderation;
mod orders;
mod requisitions;
mod routes;
mod security;

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use anyhow::Result;
use config::AppConfig;
use security::{middleware::IpGuard, rate_limit::RateLimiter, signing_mode::SigningMode};
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

    let cfg = AppConfig::load()?;
    tracing::info!(env = %cfg.environment, "starting backend-api");

    let pool = infrastructure::db::connect(&cfg.database_url).await?;

    if cfg.run_migrations {
        infrastructure::migrations::run(&pool).await?;
    }
    if cfg.run_seeds {
        infrastructure::seed::run(&pool).await?;
    }

    let bind = (cfg.server.host.clone(), cfg.server.port);
    let pool_data = web::Data::new(pool.clone());

    // Build the rate-limiter backend from config.  For `postgres` we pass the
    // connection pool so each replica can share the sliding window.  Any other
    // value (including the default `memory`) yields the process-local backend.
    let limiter = RateLimiter::from_config(
        &cfg.rate_limit_backend,
        if cfg.rate_limit_backend.eq_ignore_ascii_case("postgres") {
            Some(pool.clone())
        } else {
            None
        },
    );

    let signing_mode = SigningMode::from_config(&cfg.signing_mode);
    tracing::info!(
        host = %bind.0,
        port = bind.1,
        signing_mode = signing_mode.label(),
        rate_limit_backend = %cfg.rate_limit_backend,
        "listening",
    );

    HttpServer::new(move || {
        App::new()
            .app_data(pool_data.clone())
            .wrap(IpGuard { limiter: limiter.clone() })
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Logger::default())
            .wrap(
                Cors::default()
                    // Allow the nginx proxy origin (port 8000) and direct frontend
                    // access (port 8081) used during local-network deployment and dev.
                    .allowed_origin("http://localhost:8000")
                    .allowed_origin("http://localhost:8081")
                    .allowed_origin("http://localhost:8080")
                    .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE"])
                    .allowed_headers(vec![
                        actix_web::http::header::CONTENT_TYPE,
                        actix_web::http::header::AUTHORIZATION,
                        // Custom signed-request headers
                        actix_web::http::header::HeaderName::from_static("x-silveroak-token"),
                        actix_web::http::header::HeaderName::from_static("x-silveroak-timestamp"),
                        actix_web::http::header::HeaderName::from_static("x-silveroak-nonce"),
                        actix_web::http::header::HeaderName::from_static("x-silveroak-signature"),
                        actix_web::http::header::HeaderName::from_static("x-silveroak-body-hash"),
                    ])
                    .max_age(3600),
            )
            .configure(move |cfg| routes::configure(cfg, signing_mode))
    })
    .bind(bind)?
    .run()
    .await?;

    Ok(())
}
