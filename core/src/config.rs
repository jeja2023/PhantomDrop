use std::env;
use std::net::SocketAddr;

use axum::http::{
    HeaderValue, Method,
    header::{CONTENT_TYPE, HeaderName},
};
use tower_http::cors::{Any, CorsLayer};

pub const DEFAULT_HUB_SECRET: &str = "local_dev_secret";

#[derive(Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub cors_origins: Vec<HeaderValue>,
    pub debug_assets_enabled: bool,
    pub environment: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let environment = env::var("APP_ENV")
            .or_else(|_| env::var("RUST_ENV"))
            .unwrap_or_else(|_| "development".to_string());
        let bind_host = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(9010);
        let bind_addr = format!("{bind_host}:{port}")
            .parse::<SocketAddr>()
            .map_err(|error| format!("监听地址无效: {bind_host}:{port} ({error})"))?;

        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| {
                "http://127.0.0.1:5173,http://localhost:5173,http://127.0.0.1:9010,http://localhost:9010"
                    .to_string()
            })
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty() && *origin != "*")
            .map(|origin| {
                origin
                    .parse::<HeaderValue>()
                    .map_err(|error| format!("CORS_ORIGINS 包含无效来源 {origin}: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let debug_assets_enabled = parse_bool_env("ENABLE_DEBUG_ASSETS")
            .unwrap_or_else(|| !is_production_environment(&environment));

        Ok(Self {
            bind_addr,
            cors_origins,
            debug_assets_enabled,
            environment,
        })
    }

    pub fn cors_layer(&self) -> CorsLayer {
        let layer = if self.cors_origins.is_empty() {
            CorsLayer::new().allow_origin(Any)
        } else {
            CorsLayer::new().allow_origin(self.cors_origins.clone())
        };

        layer
            .allow_methods([Method::GET, Method::POST, Method::DELETE])
            .allow_headers([CONTENT_TYPE, HeaderName::from_static("x-hub-secret")])
    }

    pub fn is_production(&self) -> bool {
        is_production_environment(&self.environment)
    }
}

pub fn validate_hub_secret_for_environment(config: &AppConfig, secret: Option<&str>) -> Result<(), String> {
    if !config.is_production() {
        return Ok(());
    }

    match secret.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) if value != DEFAULT_HUB_SECRET => Ok(()),
        _ => Err("生产环境必须配置非默认 HUB_SECRET".to_string()),
    }
}

fn parse_bool_env(key: &str) -> Option<bool> {
    env::var(key).ok().and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    })
}

fn is_production_environment(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "prod" | "production")
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, DEFAULT_HUB_SECRET, validate_hub_secret_for_environment};

    fn config_for(environment: &str) -> AppConfig {
        AppConfig {
            bind_addr: "127.0.0.1:9010".parse().unwrap(),
            cors_origins: Vec::new(),
            debug_assets_enabled: false,
            environment: environment.to_string(),
        }
    }

    #[test]
    fn production_rejects_missing_or_default_secret() {
        let config = config_for("production");

        assert!(validate_hub_secret_for_environment(&config, None).is_err());
        assert!(validate_hub_secret_for_environment(&config, Some(DEFAULT_HUB_SECRET)).is_err());
    }

    #[test]
    fn production_accepts_non_default_secret() {
        let config = config_for("prod");

        assert!(validate_hub_secret_for_environment(&config, Some("strong-secret")).is_ok());
    }

    #[test]
    fn development_allows_default_secret() {
        let config = config_for("development");

        assert!(validate_hub_secret_for_environment(&config, Some(DEFAULT_HUB_SECRET)).is_ok());
    }
}
