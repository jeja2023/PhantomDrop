use std::env;
use std::net::SocketAddr;

use axum::http::{
    HeaderValue, Method,
    header::{CONTENT_TYPE, HeaderName},
};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppConfig {
    pub bind_addr: SocketAddr,
    pub cors_origins: Option<Vec<HeaderValue>>,
    pub debug_assets_enabled: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let environment = env::var("APP_ENV")
            .or_else(|_| env::var("RUST_ENV"))
            .unwrap_or_else(|_| "development".to_string());
        let bind_host = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(9010);
        let bind_addr = format!("{bind_host}:{port}")
            .parse::<SocketAddr>()
            .map_err(|error| format!("监听地址无效: {bind_host}:{port} ({error})"))?;

        let cors_raw = env::var("CORS_ORIGINS").unwrap_or_else(|_| {
            "http://127.0.0.1:5173,http://localhost:5173,http://127.0.0.1:9010,http://localhost:9010"
                .to_string()
        });

        let cors_origins = if cors_raw.trim() == "*" {
            None
        } else {
            let list = cors_raw
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(|origin| {
                    origin
                        .parse::<HeaderValue>()
                        .map_err(|error| format!("CORS_ORIGINS 包含无效来源 {origin}: {error}"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Some(list)
        };

        let debug_assets_enabled = parse_bool_env("ENABLE_DEBUG_ASSETS")
            .unwrap_or_else(|| !is_production_environment(&environment));

        Ok(Self {
            bind_addr,
            cors_origins,
            debug_assets_enabled,
        })
    }

    pub fn cors_layer(&self) -> CorsLayer {
        let layer = match &self.cors_origins {
            None => CorsLayer::new().allow_origin(Any),
            Some(origins) => {
                if origins.is_empty() {
                    CorsLayer::new()
                } else {
                    CorsLayer::new().allow_origin(origins.clone())
                }
            }
        };

        layer
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers([
                CONTENT_TYPE,
                HeaderName::from_static("x-hub-secret"),
                HeaderName::from_static("authorization"),
                HeaderName::from_static("x-auth-token"),
            ])
    }
}

fn parse_bool_env(key: &str) -> Option<bool> {
    env::var(key)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}

fn is_production_environment(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "prod" | "production"
    )
}
