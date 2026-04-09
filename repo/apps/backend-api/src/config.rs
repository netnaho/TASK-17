use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database_url: String,
    pub run_migrations: bool,
    pub run_seeds: bool,
    pub environment: String,
    /// Signing mode controlling which HMAC canonical-string contract is enforced.
    /// Values: `"compat"` (default, 4-field), `"dual"` (try strict first, compat
    /// fallback with WARN), `"strict"` (6-field only, requires body-hash header).
    /// See `security::signing_mode::SigningMode` for the rollout playbook.
    pub signing_mode: String,
    /// Which rate-limit backend to use: `"memory"` (default, process-local) or
    /// `"postgres"` (distributed, requires migration 0008).  Use `"postgres"`
    /// when running multiple API replicas behind a load balancer.
    pub rate_limit_backend: String,
}

impl AppConfig {
    /// Load layered config: built-in defaults, then env vars (`APP__*`),
    /// then a single override for `DATABASE_URL` so docker-compose can set
    /// it without the `APP__` prefix.
    pub fn load() -> anyhow::Result<Self> {
        let env = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());

        let builder = config::Config::builder()
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 8080)?
            .set_default("database_url", "postgres://silveroak:silveroak_dev@localhost:5432/silveroak")?
            .set_default("run_migrations", true)?
            .set_default("run_seeds", true)?
            .set_default("environment", env.clone())?
            .set_default("signing_mode", "compat")?
            .set_default("rate_limit_backend", "memory")?
            .add_source(
                config::Environment::with_prefix("APP")
                    .prefix_separator("__")
                    .separator("__"),
            );

        let mut cfg: AppConfig = builder.build()?.try_deserialize()?;
        if let Ok(url) = std::env::var("DATABASE_URL") {
            cfg.database_url = url;
        }
        Ok(cfg)
    }
}
