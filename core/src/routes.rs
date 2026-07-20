pub(crate) mod emails;

use crate::cloudflare_automation::{CloudflareAutomationManager, CloudflareAutomationRunPayload};
use crate::config::AppConfig;
use crate::db::DataLake;
use crate::stream::StreamHub;
use crate::{openai, stream, tunnel, workflow};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::extract::{Path, Query};
use axum::{
    Json, Router,
    http::{StatusCode, header::CONTENT_TYPE},
    response::{Html, IntoResponse},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::path::{Component, Path as FsPath};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tower_http::services::ServeDir;

#[derive(Deserialize)]
struct LoginPayload {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct AdminCredentialUpdatePayload {
    current_password: String,
    username: Option<String>,
    new_password: Option<String>,
}

struct LoginAttempts {
    window_started: Instant,
    failures: u32,
}

static LOGIN_ATTEMPTS: OnceLock<Mutex<HashMap<IpAddr, LoginAttempts>>> = OnceLock::new();
static SESSION_SALT: OnceLock<[u8; 32]> = OnceLock::new();

fn login_attempt_allowed(peer: IpAddr) -> bool {
    let attempts = LOGIN_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut attempts = attempts
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    attempts.retain(|_, entry| entry.window_started.elapsed() < Duration::from_secs(60));
    attempts
        .get(&peer)
        .map(|entry| entry.failures < 5)
        .unwrap_or(true)
}

fn record_login_failure(peer: IpAddr) {
    let attempts = LOGIN_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut attempts = attempts
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = attempts.entry(peer).or_insert(LoginAttempts {
        window_started: Instant::now(),
        failures: 0,
    });
    if entry.window_started.elapsed() >= Duration::from_secs(60) {
        entry.window_started = Instant::now();
        entry.failures = 0;
    }
    entry.failures += 1;
}

fn clear_login_failures(peer: IpAddr) {
    if let Some(attempts) = LOGIN_ATTEMPTS.get() {
        attempts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&peer);
    }
}

fn validate_admin_username(username: &str) -> Result<(), &'static str> {
    let length = username.chars().count();
    if !(3..=64).contains(&length) {
        return Err("用户名长度必须为 3 到 64 个字符");
    }
    if username.chars().any(char::is_control) {
        return Err("用户名不能包含控制字符");
    }
    Ok(())
}

fn validate_admin_password(password: &str) -> Result<(), &'static str> {
    if password.chars().count() < 12 {
        return Err("密码长度至少为 12 个字符");
    }
    if password.chars().count() > 256 {
        return Err("密码长度不能超过 256 个字符");
    }
    Ok(())
}

async fn hash_admin_password(password: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut rand::rngs::OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|error| format!("密码哈希失败: {error}"))
    })
    .await
    .map_err(|error| format!("密码哈希任务失败: {error}"))?
}

async fn verify_admin_password(password_hash: String, password: String) -> bool {
    tokio::task::spawn_blocking(move || {
        let Ok(parsed_hash) = PasswordHash::new(&password_hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    })
    .await
    .unwrap_or(false)
}

fn validate_legacy_auth_migration(
    legacy_auth_secret: Option<&str>,
    configured_hub_secret: Option<&str>,
) -> anyhow::Result<()> {
    let Some(legacy_secret) = legacy_auth_secret
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let Some(hub_secret) = configured_hub_secret
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        anyhow::bail!(
            "检测到旧 auth_secret。升级前请将原机器通信密钥配置为 HUB_SECRET；旧值尚未删除"
        );
    };
    if !constant_time_eq(legacy_secret.as_bytes(), hub_secret.as_bytes()) {
        anyhow::bail!(
            "HUB_SECRET 与旧 auth_secret 不一致。首次升级必须保持相同值，以免 Worker 中断"
        );
    }
    Ok(())
}

pub async fn ensure_admin_credentials(data_lake: &DataLake) -> anyhow::Result<()> {
    let legacy_auth_secret = data_lake
        .get_setting("auth_secret")
        .await?
        .filter(|value| !value.trim().is_empty());
    let configured_hub_secret = std::env::var("HUB_SECRET").ok();
    validate_legacy_auth_migration(
        legacy_auth_secret.as_deref(),
        configured_hub_secret.as_deref(),
    )?;

    if data_lake.get_admin_credentials().await?.is_some() {
        if legacy_auth_secret.is_some() {
            data_lake.delete_setting("auth_secret").await?;
        }
        return Ok(());
    }

    let username = std::env::var("ADMIN_USERNAME")
        .unwrap_or_else(|_| "admin".to_string())
        .trim()
        .to_string();
    validate_admin_username(&username).map_err(anyhow::Error::msg)?;
    let password = std::env::var("ADMIN_PASSWORD")
        .map_err(|_| anyhow::anyhow!("首次启动必须设置 ADMIN_PASSWORD（至少 12 个字符）"))?;
    validate_admin_password(&password).map_err(anyhow::Error::msg)?;
    let password_hash = hash_admin_password(password)
        .await
        .map_err(anyhow::Error::msg)?;
    data_lake
        .replace_admin_credentials(&username, &password_hash)
        .await?;
    if legacy_auth_secret.is_some() {
        data_lake.delete_setting("auth_secret").await?;
    }
    println!("管理员账户已初始化: {username}");
    Ok(())
}

async fn login_handler(
    peer: SocketAddr,
    data_lake: Arc<DataLake>,
    Json(payload): Json<LoginPayload>,
) -> axum::response::Response {
    if !login_attempt_allowed(peer.ip()) {
        return auth_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "登录尝试过于频繁，请稍后重试",
            false,
        );
    }
    let Ok(Some(credentials)) = data_lake.get_admin_credentials().await else {
        return auth_error_response(StatusCode::SERVICE_UNAVAILABLE, "管理员账户未初始化", false);
    };
    let username = payload.username.trim();
    if username.chars().count() > 64 || payload.password.chars().count() > 256 {
        record_login_failure(peer.ip());
        return auth_error_response(StatusCode::UNAUTHORIZED, "用户名或密码错误", false);
    }
    let username_matches = constant_time_eq(username.as_bytes(), credentials.username.as_bytes());
    let password_matches =
        verify_admin_password(credentials.password_hash.clone(), payload.password).await;
    if !username_matches || !password_matches {
        record_login_failure(peer.ip());
        return auth_error_response(StatusCode::UNAUTHORIZED, "用户名或密码错误", false);
    }
    clear_login_failures(peer.ip());
    let secure = std::env::var("COOKIE_SECURE")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false);
    let cookie = format!(
        "phantom_auth_token={}; Path=/; Max-Age=28800; HttpOnly; SameSite=Strict{}",
        auth_session_token(&credentials.username, &credentials.password_hash),
        if secure { "; Secure" } else { "" }
    );
    axum::response::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("Set-Cookie", cookie)
        .header("Cache-Control", "no-store")
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn update_admin_credentials_handler(
    data_lake: Arc<DataLake>,
    Json(payload): Json<AdminCredentialUpdatePayload>,
) -> axum::response::Response {
    let Ok(Some(current)) = data_lake.get_admin_credentials().await else {
        return auth_error_response(StatusCode::SERVICE_UNAVAILABLE, "管理员账户未初始化", false);
    };
    if payload.current_password.chars().count() > 256 {
        return auth_error_response(StatusCode::UNAUTHORIZED, "当前密码错误", false);
    }
    if !verify_admin_password(current.password_hash.clone(), payload.current_password).await {
        return auth_error_response(StatusCode::UNAUTHORIZED, "当前密码错误", false);
    }

    let username = payload
        .username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.username)
        .to_string();
    if let Err(message) = validate_admin_username(&username) {
        return auth_error_response(StatusCode::BAD_REQUEST, message, false);
    }

    let password_hash =
        if let Some(new_password) = payload.new_password.filter(|value| !value.is_empty()) {
            if let Err(message) = validate_admin_password(&new_password) {
                return auth_error_response(StatusCode::BAD_REQUEST, message, false);
            }
            match hash_admin_password(new_password).await {
                Ok(hash) => hash,
                Err(error) => {
                    return auth_error_response(StatusCode::INTERNAL_SERVER_ERROR, &error, false);
                }
            }
        } else {
            current.password_hash
        };

    match data_lake
        .replace_admin_credentials(&username, &password_hash)
        .await
    {
        Ok(()) => {
            Json(serde_json::json!({"status": "success", "username": username})).into_response()
        }
        Err(error) => {
            eprintln!("更新管理员凭据失败: {error:?}");
            auth_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "更新管理员凭据失败",
                false,
            )
        }
    }
}
async fn logout_handler() -> axum::response::Response {
    axum::response::Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(
            "Set-Cookie",
            "phantom_auth_token=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict",
        )
        .header("Cache-Control", "no-store")
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn auth_session_token(username: &str, password_hash: &str) -> String {
    use base64::Engine;
    use rand::RngCore;
    let salt = SESSION_SALT.get_or_init(|| {
        let mut value = [0_u8; 32];
        rand::thread_rng().fill_bytes(&mut value);
        value
    });
    let mut material = Vec::with_capacity(username.len() + password_hash.len() + salt.len() + 1);
    material.extend_from_slice(username.as_bytes());
    material.push(0);
    material.extend_from_slice(password_hash.as_bytes());
    material.extend_from_slice(salt);
    let digest = crate::openai::oauth::simple_sha256_public(&material);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}
fn constant_time_eq(expected: &[u8], provided: &[u8]) -> bool {
    let mut difference = expected.len() ^ provided.len();
    let length = expected.len().max(provided.len());
    for index in 0..length {
        difference |= usize::from(
            expected.get(index).copied().unwrap_or_default()
                ^ provided.get(index).copied().unwrap_or_default(),
        );
    }
    difference == 0
}
#[derive(Deserialize)]
struct WorkflowTrigger {
    workflow_id: String,
}

#[derive(Deserialize)]
struct WorkflowDefinitionPayload {
    id: String,
    kind: String,
    title: String,
    summary: String,
    status: String,
    parameters_json: String,
}

#[derive(Deserialize)]
struct TunnelConfig {
    port: u16,
    subdomain: Option<String>,
    public_url: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
struct SettingsPayload {
    webhook_url: Option<String>,
    update_rate: Option<u64>,
    admin_username: Option<String>,
    decode_depth: Option<String>,
    public_hub_url: Option<String>,
    account_domain: Option<String>,
    cloudflare_default_mode: Option<String>,
    cloudflare_public_url: Option<String>,
    cloudflare_route_local_part: Option<String>,
    cloudflare_zone_domain: Option<String>,
    cloudflare_api_token: Option<String>,
    cloudflare_zone_id: Option<String>,
    cloudflare_account_id: Option<String>,
    cpa_url: Option<String>,
    cpa_key: Option<String>,
    sub2api_url: Option<String>,
    sub2api_key: Option<String>,
    cpa_auth_json: Option<String>,
}

#[derive(Deserialize, Default)]
struct WorkflowRunQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    status: Option<String>,
    workflow_id: Option<String>,
    workflow_exact: Option<bool>,
}

#[derive(Deserialize)]
struct UpdatePoolPayload {
    ids: Vec<String>,
    pool_tag: String,
}
#[derive(Serialize)]
struct AccountSummary {
    id: String,
    run_id: String,
    address: String,
    status: String,
    created_at: i64,
    upload_status: Option<String>,
    account_type: Option<String>,
    pool_tag: Option<String>,
    last_used_at: Option<i64>,
    rate_limit_reset_at: Option<i64>,
    consecutive_failures: Option<i64>,
    request_count_24h: Option<i64>,
    last_failure_reason: Option<String>,
    proxy_rtt: Option<i64>,
    proxy_ip_type: Option<String>,
    proxy_status: Option<String>,
    proxy_last_checked_at: Option<i64>,
}

fn account_summary(account: crate::db::GeneratedAccountRecord) -> AccountSummary {
    AccountSummary {
        id: account.id,
        run_id: account.run_id,
        address: account.address,
        status: account.status,
        created_at: account.created_at,
        upload_status: account.upload_status,
        account_type: account.account_type,
        pool_tag: account.pool_tag,
        last_used_at: account.last_used_at,
        rate_limit_reset_at: account.rate_limit_reset_at,
        consecutive_failures: account.consecutive_failures,
        request_count_24h: account.request_count_24h,
        last_failure_reason: account.last_failure_reason,
        proxy_rtt: account.proxy_rtt,
        proxy_ip_type: account.proxy_ip_type,
        proxy_status: account.proxy_status,
        proxy_last_checked_at: account.proxy_last_checked_at,
    }
}

async fn console_index() -> Html<&'static str> {
    Html(include_str!("../console/index.html"))
}

async fn console_style() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../console/style.css"),
    )
}

async fn console_script() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("../console/app.js"),
    )
}

async fn debug_asset(name: String, enabled: bool) -> axum::response::Response {
    if !enabled {
        return (StatusCode::NOT_FOUND, "Debug assets are disabled").into_response();
    }

    if name.is_empty()
        || !name.ends_with(".png")
        || FsPath::new(&name)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return (StatusCode::BAD_REQUEST, "Invalid debug asset name").into_response();
    }

    let path = FsPath::new("./data").join(name);
    if !path.exists() || !path.is_file() {
        return (StatusCode::NOT_FOUND, "Screenshot not found").into_response();
    }

    let content = match std::fs::read(&path) {
        Ok(content) => content,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Read error").into_response(),
    };

    axum::response::Response::builder()
        .header("Content-Type", "image/png")
        .body(axum::body::Body::from(content))
        .unwrap_or_else(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response build error").into_response()
        })
}

pub async fn validate_ssrf_url(url_str: &str) -> Result<url::Url, String> {
    build_ssrf_safe_client(url_str).await.map(|(url, _)| url)
}

pub async fn build_ssrf_safe_client(url_str: &str) -> Result<(url::Url, reqwest::Client), String> {
    let parsed = url::Url::parse(url_str).map_err(|error| format!("URL 格式无效: {error}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "URL 缺少主机名".to_string())?
        .to_string();
    let is_local_http = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1");
    match parsed.scheme() {
        "https" => {}
        "http" if is_local_http => {}
        _ => return Err("仅支持 HTTPS 或本机 HTTP 地址".to_string()),
    }
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15));
    if !is_local_http {
        let port = parsed.port_or_known_default().unwrap_or(443);
        let addresses: Vec<SocketAddr> = tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::lookup_host((host.as_str(), port)),
        )
        .await
        .map_err(|_| "DNS 解析超时".to_string())?
        .map_err(|error| format!("DNS 解析失败: {error}"))?
        .collect();
        if addresses.is_empty() {
            return Err("DNS 未返回可用地址".to_string());
        }
        if addresses.iter().any(|address| is_intranet_ip(address.ip())) {
            return Err("目标地址解析到了内网或保留 IP".to_string());
        }
        builder = builder.resolve_to_addrs(&host, &addresses);
    }
    let client = builder
        .build()
        .map_err(|error| format!("HTTP 客户端初始化失败: {error}"))?;
    Ok((parsed, client))
}
fn is_intranet_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            let [first, second, ..] = ipv4.octets();
            ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_unspecified()
                || ipv4.is_broadcast()
                || ipv4.is_documentation()
                || ipv4.is_multicast()
                || first == 0
                || (first == 100 && (64..=127).contains(&second))
                || (first == 198 && (18..=19).contains(&second))
                || first >= 240
        }
        std::net::IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.is_multicast()
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
                || ipv6.segments()[0] == 0x2001 && ipv6.segments()[1] == 0x0db8
        }
    }
}
fn extract_cookie_token(cookie_header: &str) -> Option<String> {
    for cookie in cookie_header.split(';') {
        let mut parts = cookie.trim().splitn(2, '=');
        if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
            if key == "phantom_auth_token" {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn auth_error_response(status: StatusCode, message: &str, html: bool) -> axum::response::Response {
    if !html {
        return axum::response::Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                serde_json::json!({"status": "error", "message": message}).to_string(),
            ))
            .unwrap_or_else(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, "Response build error").into_response()
            });
    }

    let login_html = r#"<!DOCTYPE html><html><body><p>Authentication required.</p></body></html>"#;
    axum::response::Response::builder()
        .status(status)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(login_html))
        .unwrap_or_else(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response build error").into_response()
        })
}

async fn auth_middleware(
    axum::extract::State(data_lake): axum::extract::State<Arc<DataLake>>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let path = req.uri().path();

    if path == "/health"
        || path == "/ingest"
        || path == "/v1/chat/completions"
        || path == "/auth/login"
    {
        return Ok(next.run(req).await);
    }

    let is_protected = path.starts_with("/api/")
        || path == "/stream"
        || path == "/stream/"
        || path.starts_with("/console")
        || path.starts_with("/debug/");

    if is_protected {
        let wants_html = !(path.starts_with("/api/") || path.starts_with("/stream"));
        let Ok(Some(credentials)) = data_lake.get_admin_credentials().await else {
            return Ok(auth_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "管理员账户未初始化，管理接口已锁定",
                wants_html,
            ));
        };
        let provided_token = req
            .headers()
            .get("cookie")
            .and_then(|header| header.to_str().ok())
            .and_then(extract_cookie_token);
        let expected_token = auth_session_token(&credentials.username, &credentials.password_hash);
        let authenticated = provided_token
            .as_deref()
            .map(|token| constant_time_eq(token.as_bytes(), expected_token.as_bytes()))
            .unwrap_or(false);

        if !authenticated {
            return Ok(auth_error_response(
                StatusCode::UNAUTHORIZED,
                "登录已失效，请使用管理员用户名和密码重新登录",
                wants_html,
            ));
        }
    }

    Ok(next.run(req).await)
}
#[allow(dead_code)]
fn mask_credential(val: Option<String>) -> Option<String> {
    val.map(|s| {
        if s.trim().is_empty() {
            "".to_string()
        } else {
            "******".to_string()
        }
    })
}

fn settings_from_map(map: HashMap<String, String>) -> SettingsPayload {
    SettingsPayload {
        webhook_url: map.get("webhook_url").cloned().filter(|v| !v.is_empty()),
        update_rate: map.get("update_rate").and_then(|v| v.parse::<u64>().ok()),
        admin_username: map.get("admin_username").cloned().filter(|v| !v.is_empty()),
        decode_depth: map.get("decode_depth").cloned().filter(|v| !v.is_empty()),
        public_hub_url: map.get("public_hub_url").cloned().filter(|v| !v.is_empty()),
        account_domain: map.get("account_domain").cloned().filter(|v| !v.is_empty()),
        cloudflare_default_mode: map
            .get("cloudflare_default_mode")
            .cloned()
            .filter(|v| !v.is_empty()),
        cloudflare_public_url: map
            .get("cloudflare_public_url")
            .cloned()
            .filter(|v| !v.is_empty()),
        cloudflare_route_local_part: map
            .get("cloudflare_route_local_part")
            .cloned()
            .filter(|v| !v.is_empty()),
        cloudflare_zone_domain: map
            .get("cloudflare_zone_domain")
            .cloned()
            .filter(|v| !v.is_empty()),
        cloudflare_api_token: mask_credential(
            map.get("cloudflare_api_token")
                .cloned()
                .filter(|v| !v.is_empty()),
        ),
        cloudflare_zone_id: map
            .get("cloudflare_zone_id")
            .cloned()
            .filter(|v| !v.is_empty()),
        cloudflare_account_id: map
            .get("cloudflare_account_id")
            .cloned()
            .filter(|v| !v.is_empty()),
        cpa_url: map.get("cpa_url").cloned().filter(|v| !v.is_empty()),
        cpa_key: mask_credential(map.get("cpa_key").cloned().filter(|v| !v.is_empty())),
        sub2api_url: map.get("sub2api_url").cloned().filter(|v| !v.is_empty()),
        sub2api_key: mask_credential(map.get("sub2api_key").cloned().filter(|v| !v.is_empty())),
        cpa_auth_json: mask_credential(map.get("cpa_auth_json").cloned().filter(|v| !v.is_empty())),
    }
}

pub struct RouterContext {
    pub data_lake: Arc<DataLake>,
    pub stream_hub: Arc<StreamHub>,
    pub workflow_engine: Arc<workflow::WorkflowEngine>,
    pub tunnel_manager: Arc<tunnel::TunnelManager>,
    pub automation_manager: Arc<CloudflareAutomationManager>,
    pub app_config: AppConfig,
    pub web_dist: String,
}

pub fn build_router(ctx: RouterContext) -> Router {
    let data_lake = ctx.data_lake;
    let stream_hub = ctx.stream_hub;
    let workflow_engine = ctx.workflow_engine;
    let tunnel_manager = ctx.tunnel_manager;
    let automation_manager = ctx.automation_manager;
    let app_config = ctx.app_config;
    let web_dist = ctx.web_dist;

    Router::new()
        .route("/auth/login", post({
            let dl = Arc::clone(&data_lake);
            move |axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<SocketAddr>,
                  Json(payload): Json<LoginPayload>| {
                let dl = dl.clone();
                async move { login_handler(peer, dl, Json(payload)).await }
            }
        }))
        .route("/auth/logout", post(logout_handler))
        .route("/api/admin/credentials", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<AdminCredentialUpdatePayload>| {
                let dl = dl.clone();
                async move { update_admin_credentials_handler(dl, Json(payload)).await }
            }
        }))
        .route("/v1/chat/completions", post({
            let dl = Arc::clone(&data_lake);
            move |req: axum::extract::Request| {
                let dl = dl.clone();
                async move {
                    crate::openai::gateway::chat_completions_gateway(dl, req).await
                }
            }
        }))
        .route("/console", get(console_index))
        .route("/console/", get(console_index))
        .route("/console/style.css", get(console_style))
        .route("/console/app.js", get(console_script))
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .merge(emails::routes(Arc::clone(&data_lake), Arc::clone(&stream_hub)))
        .route("/api/settings", get({
            let dl = Arc::clone(&data_lake);
            move || {
                let dl = dl.clone();
                async move {
                    match dl.list_settings().await {
                        Ok(settings) => Json(settings_from_map(settings)).into_response(),
                        Err(e) => {
                            eprintln!("读取配置失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取配置失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/stats", get({
            let dl = Arc::clone(&data_lake);
            move || {
                let dl = dl.clone();
                async move {
                    match dl.get_dashboard_stats().await {
                        Ok(stats) => Json(stats).into_response(),
                        Err(e) => {
                            eprintln!("读取统计失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取统计失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/workflows", get({
            let engine = Arc::clone(&workflow_engine);
            move || {
                let engine = engine.clone();
                async move {
                    Json(engine.definitions().await)
                }
            }
        }))
        .route("/api/workflows/grok/readiness", get({
            let engine = Arc::clone(&workflow_engine);
            move || {
                let engine = engine.clone();
                async move { Json(engine.grok_readiness().await) }
            }
        }))
        .route("/api/workflows/save", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<WorkflowDefinitionPayload>| {
                let dl = dl.clone();
                async move {
                    let builtin_ids = workflow::WorkflowEngine::builtin_ids();
                    let is_builtin = builtin_ids.iter().any(|builtin_id| builtin_id == &payload.id.as_str());
                    if is_builtin {
                        let builtin_kind = workflow::WorkflowEngine::builtin_definitions()
                            .into_iter()
                            .find(|definition| definition.id == payload.id)
                            .map(|definition| definition.kind.as_storage().to_string());
                        if let Some(expected_kind) = builtin_kind {
                            if payload.kind != expected_kind {
                                return (
                                    StatusCode::BAD_REQUEST,
                                    Json(serde_json::json!({"status": "error", "message": "内置工作流不允许修改 kind"}))
                                ).into_response();
                            }
                        }
                    }

                    if let Err(message) = workflow::WorkflowEngine::validate_definition_input(
                        &payload.kind,
                        &payload.status,
                        &payload.parameters_json,
                    ) {
                        eprintln!("❌ 工作流保存预校验失败:");
                        eprintln!(" - ID: {}", payload.id);
                        eprintln!(" - Kind: {}", payload.kind);
                        eprintln!(" - 原因: {message}");
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({"status": "error", "message": message}))
                        ).into_response();
                    }

                    match dl.upsert_workflow_definition(
                        &payload.id,
                        &payload.kind,
                        &payload.title,
                        &payload.summary,
                        &payload.status,
                        &payload.parameters_json,
                    ).await {
                        Ok(_) => Json(serde_json::json!({"status": "success"})).into_response(),
                        Err(e) => {
                            eprintln!("保存工作流定义失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "保存工作流定义失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/workflows/:id", delete({
            let dl = Arc::clone(&data_lake);
            move |Path(id): Path<String>| {
                let dl = dl.clone();
                async move {
                    if workflow::WorkflowEngine::builtin_ids().iter().any(|builtin_id| builtin_id == &id.as_str()) {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({"status": "error", "message": "内置工作流不允许删除"}))
                        ).into_response();
                    }

                    match dl.delete_workflow_definition(&id).await {
                        Ok(count) if count > 0 => Json(serde_json::json!({"status": "success"})).into_response(),
                        Ok(_) => (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"status": "error", "message": "工作流不存在"}))
                        ).into_response(),
                        Err(e) => {
                            eprintln!("删除工作流定义失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "删除工作流定义失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/workflow-runs", get({
            let dl = Arc::clone(&data_lake);
            move |Query(query): Query<WorkflowRunQuery>| {
                let dl = dl.clone();
                async move {
                    let page = query.page.unwrap_or(1);
                    let page_size = query.page_size.unwrap_or(20);
                    match dl.get_workflow_runs_page(
                        page,
                        page_size,
                        query.status.as_deref(),
                        query.workflow_id.as_deref(),
                        query.workflow_exact.unwrap_or(false),
                    ).await {
                        Ok(result) => Json(result).into_response(),
                        Err(e) => {
                            eprintln!("读取工作流执行记录失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取工作流执行记录失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/workflow-runs/:run_id/steps", get({
            let dl = Arc::clone(&data_lake);
            move |Path(run_id): Path<String>| {
                let dl = dl.clone();
                async move {
                    match dl.list_workflow_steps(&run_id, 100).await {
                        Ok(steps) => Json(steps).into_response(),
                        Err(e) => {
                            eprintln!("读取工作流步骤失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取工作流步骤失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/workflow-runs/:run_id/stop", post({
            let dl = Arc::clone(&data_lake);
            move |Path(run_id): Path<String>| {
                let dl = dl.clone();
                async move {
                    match dl.stop_workflow_run(&run_id).await {
                        Ok(count) if count > 0 => Json(serde_json::json!({"status": "success"})).into_response(),
                        Ok(_) => (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"status": "error", "message": "工作流未运行或未找到"}))
                        ).into_response(),
                        Err(e) => {
                            eprintln!("停止工作流失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "停止工作流失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/workflow-runs/:run_id/accounts", get({
            let dl = Arc::clone(&data_lake);
            move |Path(run_id): Path<String>| {
                let dl = dl.clone();
                async move {
                    match dl.list_generated_accounts(&run_id, 200).await {
                        Ok(accounts) => Json(accounts.into_iter().map(account_summary).collect::<Vec<_>>()).into_response(),
                        Err(e) => {
                            eprintln!("读取生成账号失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取生成账号失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/workflow-runs/:run_id/accounts/export", get({
            let dl = Arc::clone(&data_lake);
            move |Path(run_id): Path<String>| {
                let dl = dl.clone();
                async move {
                    match dl.list_generated_accounts(&run_id, 1000).await {
                        Ok(accounts) => {
                            let mut csv = String::from("address,password,status,created_at,access_token,session_token,refresh_token\n");
                            for account in accounts {
                                let line = format!(
                                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                                    account.address.replace('"', "\"\""),
                                    account.password.replace('"', "\"\""),
                                    account.status.replace('"', "\"\""),
                                    chrono::DateTime::from_timestamp(account.created_at, 0)
                                        .map(|dt| dt.naive_utc().to_string())
                                        .unwrap_or_else(|| account.created_at.to_string()),
                                    account.access_token.as_deref().unwrap_or("").replace('"', "\"\""),
                                    account.session_token.as_deref().unwrap_or("").replace('"', "\"\""),
                                    account.refresh_token.as_deref().unwrap_or("").replace('"', "\"\""),
                                );
                                csv.push_str(&line);
                            }
                            (
                                [(CONTENT_TYPE, "text/csv; charset=utf-8")],
                                format!("\u{FEFF}{csv}"),
                            ).into_response()
                        }
                        Err(e) => {
                            eprintln!("导出生成账号失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "导出生成账号失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/stream", get(stream::sse_handler))
        .route("/api/otp/poll", get({
            let dl = Arc::clone(&data_lake);
            move |Query(query): Query<HashMap<String, String>>| {
                let dl = dl.clone();
                async move {
                    let email = query.get("email").cloned().unwrap_or_default();
                    let since = query.get("since")
                        .and_then(|v| v.parse::<i64>().ok())
                        .unwrap_or(0);

                    if email.trim().is_empty() {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({"status": "error", "message": "缺少 email 参数"}))
                        ).into_response();
                    }

                    match dl.poll_otp_by_email(&email, since).await {
                        Ok(Some(code)) => Json(serde_json::json!({
                            "status": "found",
                            "code": code,
                            "email": email,
                        })).into_response(),
                        Ok(None) => Json(serde_json::json!({
                            "status": "pending",
                            "email": email,
                        })).into_response(),
                        Err(e) => {
                            eprintln!("OTP 轮询失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "OTP 轮询失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/accounts", get({
            let dl = Arc::clone(&data_lake);
            move |Query(query): Query<HashMap<String, String>>| {
                let dl = dl.clone();
                async move {
                    let limit = query.get("limit")
                        .and_then(|v| v.parse::<i64>().ok())
                        .unwrap_or(50);
                    let offset = query.get("offset")
                        .and_then(|v| v.parse::<i64>().ok())
                        .unwrap_or(0);
                    let q = query.get("q").cloned();

                    let items = dl.list_all_accounts(limit, offset, q.as_deref()).await;
                    let total = dl.count_all_accounts(q.as_deref()).await;

                    match (items, total) {
                        (Ok(items), Ok(total)) => Json(serde_json::json!({
                            "items": items.into_iter().map(account_summary).collect::<Vec<_>>(),
                            "limit": limit,
                            "offset": offset,
                            "total": total,
                        })).into_response(),
                        (Err(e), _) | (_, Err(e)) => {
                            eprintln!("读取全局账号列表失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取全局账号列表失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/accounts/ids", get({
            let dl = Arc::clone(&data_lake);
            move |Query(query): Query<HashMap<String, String>>| {
                let dl = dl.clone();
                async move {
                    let q = query.get("q").cloned();
                    match dl.list_all_account_ids(q.as_deref()).await {
                        Ok(ids) => Json(serde_json::json!({
                            "status": "success",
                            "ids": ids
                        })).into_response(),
                        Err(e) => {
                            eprintln!("读取全部账号 ID 失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取全部账号 ID 失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/accounts/:id/tokens", post({
            let dl = Arc::clone(&data_lake);
            move |Path(id): Path<String>, Json(payload): Json<HashMap<String, String>>| {
                let dl = dl.clone();
                async move {
                    let existing = match dl.get_generated_account(&id).await {
                        Ok(Some(account)) => account,
                        Ok(None) => {
                            return (
                                StatusCode::NOT_FOUND,
                                Json(serde_json::json!({"status": "error", "message": "账号不存在"}))
                            ).into_response();
                        }
                        Err(e) => {
                            eprintln!("读取账号 Token 失败: {e:?}");
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取账号 Token 失败"}))
                            ).into_response();
                        }
                    };

                    let access_token = payload
                        .get("access_token")
                        .map(String::as_str)
                        .and_then(|value| crate::openai::oauth::parse_non_empty(value))
                        .or(existing.access_token.as_deref());
                    let refresh_token = payload
                        .get("refresh_token")
                        .map(String::as_str)
                        .and_then(|value| crate::openai::oauth::parse_non_empty(value))
                        .or(existing.refresh_token.as_deref());
                    let session_token = payload
                        .get("session_token")
                        .map(String::as_str)
                        .and_then(|value| crate::openai::oauth::parse_non_empty(value))
                        .or(existing.session_token.as_deref());
                    let device_id = payload
                        .get("device_id")
                        .map(String::as_str)
                        .and_then(|value| crate::openai::oauth::parse_non_empty(value))
                        .or(existing.device_id.as_deref());
                    let workspace_id = payload
                        .get("workspace_id")
                        .map(String::as_str)
                        .and_then(|value| crate::openai::oauth::parse_non_empty(value))
                        .or(existing.workspace_id.as_deref());
                    let id_token = payload
                        .get("id_token")
                        .map(String::as_str)
                        .and_then(|value| crate::openai::oauth::parse_non_empty(value))
                        .or(existing.id_token.as_deref());

                    let auth_info = id_token.map(|idt| crate::openai::oauth::extract_auth_info_from_jwt(idt));
                    let chatgpt_account_id = auth_info
                        .as_ref()
                        .and_then(|info| info.chatgpt_account_id.as_deref())
                        .or(existing.chatgpt_account_id.as_deref());
                    let chatgpt_user_id = auth_info
                        .as_ref()
                        .and_then(|info| info.chatgpt_user_id.as_deref())
                        .or(existing.chatgpt_user_id.as_deref());
                    let organization_id = auth_info
                        .as_ref()
                        .and_then(|info| info.organization_id.as_deref())
                        .or(existing.organization_id.as_deref());
                    let plan_type = auth_info
                        .as_ref()
                        .and_then(|info| info.plan_type.as_deref())
                        .or(existing.plan_type.as_deref());
                    let account_email = payload
                        .get("email")
                        .map(String::as_str)
                        .and_then(|value| crate::openai::oauth::parse_non_empty(value))
                        .unwrap_or(existing.address.as_str());
                    let expires_in = payload
                        .get("expires_in")
                        .and_then(|value| value.parse::<i64>().ok())
                        .or(existing.expires_in)
                        .unwrap_or(crate::openai::oauth::DEFAULT_OAUTH_EXPIRES_IN);
                    let token_version = payload
                        .get("_token_version")
                        .or_else(|| payload.get("token_version"))
                        .and_then(|value| value.parse::<i64>().ok())
                        .or(existing.token_version)
                        .unwrap_or(crate::openai::oauth::DEFAULT_OAUTH_TOKEN_VERSION);
                    let stored_credentials = existing
                        .oauth_credentials_json
                        .as_deref()
                        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
                    let oauth_credentials = crate::openai::oauth::build_oauth_credentials(
                        crate::openai::oauth::OAuthCredentialInput {
                            email: account_email,
                            access_token,
                            refresh_token,
                            id_token,
                            workspace_id,
                            chatgpt_account_id,
                            chatgpt_user_id,
                            organization_id,
                            plan_type,
                            expires_in: Some(expires_in),
                            token_version: Some(token_version),
                            stored_credentials: stored_credentials.as_ref(),
                        },
                    );
                    let access_token = crate::openai::oauth::parse_non_empty(
                        oauth_credentials.access_token.as_str(),
                    );
                    let refresh_token = crate::openai::oauth::parse_non_empty(
                        oauth_credentials.refresh_token.as_str(),
                    );
                    let id_token = crate::openai::oauth::parse_non_empty(
                        oauth_credentials.id_token.as_str(),
                    );
                    let chatgpt_account_id = crate::openai::oauth::parse_non_empty(
                        oauth_credentials.chatgpt_account_id.as_str(),
                    );
                    let chatgpt_user_id = crate::openai::oauth::parse_non_empty(
                        oauth_credentials.chatgpt_user_id.as_str(),
                    );
                    let organization_id = crate::openai::oauth::parse_non_empty(
                        oauth_credentials.organization_id.as_str(),
                    );

                    match dl.update_account_tokens(
                        &id,
                        access_token,
                        refresh_token,
                        session_token,
                        device_id,
                        workspace_id,
                        id_token,
                        chatgpt_account_id,
                        chatgpt_user_id,
                        organization_id,
                        Some(oauth_credentials.plan_type.as_str()),
                        Some(oauth_credentials.expires_in),
                        Some(oauth_credentials.token_version),
                        oauth_credentials.json.as_deref(),
                    ).await {
                        Ok(_) => Json(serde_json::json!({"status": "success"})).into_response(),
                        Err(e) => {
                            eprintln!("更新账号 Token 失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "更新账号 Token 失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/accounts/:id", delete({
            let dl = Arc::clone(&data_lake);
            move |Path(id): Path<String>| {
                let dl = dl.clone();
                async move {
                    match dl.delete_generated_account(&id).await {
                        Ok(count) if count > 0 => Json(serde_json::json!({"status": "success"})).into_response(),
                        Ok(_) => (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"status": "error", "message": "账号不存在"}))
                        ).into_response(),
                        Err(e) => {
                            eprintln!("删除账号失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "删除账号失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/accounts/:id/check-status", post({
            let dl = Arc::clone(&data_lake);
            move |Path(id): Path<String>| {
                let dl = dl.clone();
                async move {
                    match openai::checker::check_account_status(dl, &id).await {
                        Ok(status) => Json(serde_json::json!({"status": "success", "account_status": status})).into_response(),
                        Err(e) => {
                            eprintln!("检查账号状态失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": e}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/accounts/batch/check-status", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<HashMap<String, Vec<String>>>| {
                let dl = dl.clone();
                async move {
                    let ids = payload.get("ids").cloned().unwrap_or_default();
                    let mut results = Vec::new();
                    for id in ids {
                        // 串行检查以避免触发 OpenAI 并发封禁
                        let res = openai::checker::check_account_status(dl.clone(), &id).await;
                        results.push(serde_json::json!({
                            "id": id,
                            "status": match res {
                                Ok(s) => s,
                                Err(e) => format!("Error: {e}"),
                            }
                        }));
                    }
                    Json(serde_json::json!({"status": "success", "results": results})).into_response()
                }
            }
        }))
        .route("/api/accounts/batch", delete({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<HashMap<String, Vec<String>>>| {
                let dl = dl.clone();
                async move {
                    let ids = payload.get("ids").cloned().unwrap_or_default();
                    match dl.delete_generated_accounts(&ids).await {
                        Ok(count) => Json(serde_json::json!({"status": "success", "deleted": count})).into_response(),
                        Err(e) => {
                            eprintln!("批量删除账号失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "批量删除账号失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/accounts/batch/update-pool", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<UpdatePoolPayload>| {
                let dl = dl.clone();
                async move {
                    let ids = payload.ids;
                    let pool_tag = payload.pool_tag.trim().to_string();
                    let mut success_count = 0;
                    for id in ids {
                        if dl.update_account_pool_tag(&id, &pool_tag).await.is_ok() {
                            success_count += 1;
                        }
                    }
                    Json(serde_json::json!({
                        "status": "success",
                        "message": format!("成功更新 {} 条账号的分池标签为: {}", success_count, pool_tag)
                    }))
                }
            }
        }))
        .route("/api/accounts/cleanup-failures", post({
            let dl = Arc::clone(&data_lake);
            move || {
                let dl = dl.clone();
                async move {
                    match dl.delete_failed_accounts().await {
                        Ok(count) => Json(serde_json::json!({"status": "success", "deleted": count})).into_response(),
                        Err(e) => {
                            eprintln!("清理失败账号失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "清理失败账号失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/accounts/batch/upload-cpa", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<HashMap<String, Vec<String>>>| {
                let dl = dl.clone();
                async move {
                    let ids = payload.get("ids").cloned().unwrap_or_default();

                    // 获取设置
                    let settings = match dl.list_settings().await {
                        Ok(s) => s,
                        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "无法读取设置").into_response(),
                    };

                    let mut cpa_url = match settings.get("cpa_url") {
                         Some(u) if !u.trim().is_empty() => u.trim().to_string(),
                         _ => return (StatusCode::BAD_REQUEST, "请先在设置中配置 CPA 接口地址").into_response(),
                    };

                    // 自动补全路径 (针对 CLIProxyAPI)
                    if !cpa_url.contains("/v0/") && !cpa_url.contains("/api/") {
                        cpa_url = format!("{}/v0/management/auth-files", cpa_url.trim_end_matches('/'));
                    }

                    let mut cpa_key = settings.get("cpa_key").cloned().unwrap_or_default();

                    // 增强逻辑：如果配置中 cpa_key 为空，则尝试使用 Codex OAuth 授权得到的令牌
                    if cpa_key.trim().is_empty() {
                        if let Ok(Some(auth_json)) = dl.get_setting("cpa_auth_json").await {
                            if let Ok(auth_data) = serde_json::from_str::<openai::oauth::CodexAuthData>(&auth_json) {
                                cpa_key = auth_data.access_token;
                            }
                        }
                    }

                    if cpa_key.trim().is_empty() {
                         return (StatusCode::BAD_REQUEST, "请求失败：未配置 CPA 密钥且未进行 Codex 授权").into_response();
                    }

                    let client = match build_ssrf_safe_client(&cpa_url).await {
                        Ok((_, client)) => client,
                        Err(error) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                format!("CPA 接口地址安全校验失败: {error}"),
                            )
                                .into_response();
                        }
                    };
                    let mut success_count = 0;
                    let mut fail_count = 0;

                    for id in ids {
                        if let Ok(Some(acc)) = dl.get_generated_account(&id).await {
                             let payload = crate::exporter::AccountExporter::transform(&acc, crate::exporter::ExportFormat::Cpa);
                             match crate::uploader::upload_account_multipart(
                                 &client,
                                 &cpa_url,
                                 &cpa_key,
                                 payload
                             ).await {
                                 Ok(_) => {
                                     let _ = dl.update_account_upload_status(&id, "uploaded_cpa").await;
                                     success_count += 1;
                                 },
                                 Err(e) => {
                                     eprintln!("CPA 上传失败 ({id}): {e}");
                                     fail_count += 1;
                                 }
                             }
                        }
                    }

                    Json(serde_json::json!({
                        "status": "success",
                        "message": format!("成功 {} 条, 失败 {} 条", success_count, fail_count),
                        "success_count": success_count,
                        "fail_count": fail_count
                    })).into_response()
                }
            }
        }))
        .route("/api/accounts/batch/upload-sub2api", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<HashMap<String, Vec<String>>>| {
                let dl = dl.clone();
                async move {
                    let ids = payload.get("ids").cloned().unwrap_or_default();
                    let settings = dl.list_settings().await.unwrap_or_default();

                    let sub2api_url = match settings.get("sub2api_url") {
                         Some(u) if !u.trim().is_empty() => u.clone(),
                         _ => return (StatusCode::BAD_REQUEST, "请先在设置中配置 Sub2API 接口地址").into_response(),
                    };

                    let sub2api_key = settings.get("sub2api_key").cloned().unwrap_or_default();

                    let client = match build_ssrf_safe_client(&sub2api_url).await {
                        Ok((_, client)) => client,
                        Err(error) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                format!("Sub2API 地址安全校验失败: {error}"),
                            )
                                .into_response();
                        }
                    };
                    let mut success_count = 0;
                    let mut fail_count = 0;

                    for id in ids {
                        if let Ok(Some(acc)) = dl.get_generated_account(&id).await {
                             let payload = crate::exporter::AccountExporter::transform(&acc, crate::exporter::ExportFormat::Sub2api);
                             match crate::uploader::upload_account_json(
                                 &client,
                                 &sub2api_url,
                                 &sub2api_key,
                                 payload
                             ).await {
                                 Ok(_) => {
                                     let _ = dl.update_account_upload_status(&id, "uploaded_sub2api").await;
                                     success_count += 1;
                                 },
                                 Err(e) => {
                                     eprintln!("Sub2API 上传失败 ({id}): {e}");
                                     fail_count += 1;
                                 }
                             }
                        }
                    }

                    Json(serde_json::json!({
                        "status": "success",
                        "message": format!("成功 {} 条, 失败 {} 条", success_count, fail_count),
                        "success_count": success_count,
                        "fail_count": fail_count
                    })).into_response()
                }
            }
        }))
        .route("/api/accounts/batch/export", post({
            let dl = Arc::clone(&data_lake);
            move |Query(params): Query<HashMap<String, String>>, Json(payload): Json<HashMap<String, Vec<String>>>| {
                let dl = dl.clone();
                async move {
                    let ids = payload.get("ids").cloned().unwrap_or_default();
                    let mut results = Vec::new();
                    for id in ids {
                        if let Ok(Some(acc)) = dl.get_generated_account(&id).await {
                            results.push(acc);
                        }
                    }

                    let format_param = params.get("format").map(|s| s.as_str());
                    match format_param {
                        Some("oauth") => {
                            let oauth_json = crate::exporter::AccountExporter::export_to_oauth_json(&results);
                            Json(oauth_json).into_response()
                        }
                        Some("sub2api") => {
                            let transformed: Vec<serde_json::Value> = results.iter()
                                .map(|acc| crate::exporter::AccountExporter::transform(acc, crate::exporter::ExportFormat::Sub2api))
                                .collect();
                            Json(transformed).into_response()
                        }
                        Some("cpa") => {
                            let transformed: Vec<serde_json::Value> = results.iter()
                                .map(|acc| crate::exporter::AccountExporter::transform(acc, crate::exporter::ExportFormat::Cpa))
                                .collect();
                            Json(transformed).into_response()
                        }
                        Some("kiro_go") => {
                            let transformed: Vec<serde_json::Value> = results.iter()
                                .map(|acc| crate::exporter::AccountExporter::transform(acc, crate::exporter::ExportFormat::KiroGo))
                                .collect();
                            Json(transformed).into_response()
                        }
                        _ => {
                            Json(results).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/workflows/trigger", post({
            let engine = Arc::clone(&workflow_engine);
            move |Json(payload): Json<WorkflowTrigger>| {
                let engine = engine.clone();
                async move {
                    match engine.execute(&payload.workflow_id).await {
                        Ok(run_id) => (
                            StatusCode::ACCEPTED,
                            Json(serde_json::json!({
                                "status": "accepted",
                                "msg": "指令已下发到幻影工作流引擎",
                                "run_id": run_id
                            }))
                        ).into_response(),
                        Err(message) => (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({
                                "status": "error",
                                "message": message
                            }))
                        ).into_response(),
                    }
                }
            }
        }))
        .route("/api/tunnel/status", get({
            let tm = Arc::clone(&tunnel_manager);
            move || {
                let tm = tm.clone();
                async move {
                    Json(tm.get_status())
                }
            }
        }))
        .route("/api/tunnel/start", post({
            let tm = Arc::clone(&tunnel_manager);
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<TunnelConfig>| {
                let tm = tm.clone();
                let dl = dl.clone();
                async move {
                    match tm.start(payload.port, payload.subdomain, payload.public_url).await {
                        Ok(url) => {
                            if let Err(e) = dl.upsert_setting("public_hub_url", &url).await {
                                eprintln!("保存公网地址失败: {e:?}");
                            }
                            if let Err(e) = dl.upsert_setting("public_hub_port", &payload.port.to_string()).await {
                                eprintln!("保存公网端口失败: {e:?}");
                            }
                            Json(serde_json::json!({"status": "success", "url": url})).into_response()
                        }
                        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"status": "error", "message": e}))).into_response()
                    }
                }
            }
        }))
        .route("/api/tunnel/stop", post({
            let tm = Arc::clone(&tunnel_manager);
            let dl = Arc::clone(&data_lake);
            move || {
                let tm = tm.clone();
                let dl = dl.clone();
                async move {
                    tm.stop().await;
                    if let Err(e) = dl.upsert_setting("public_hub_url", "").await {
                        eprintln!("清理公网地址失败: {e:?}");
                    }
                    Json(serde_json::json!({"status": "success"}))
                }
            }
        }))
        .route("/api/settings/save", post({
            let dl = Arc::clone(&data_lake);
            let tm = Arc::clone(&tunnel_manager);
            move |Json(payload): Json<SettingsPayload>| {
                let dl = dl.clone();
                let tm = tm.clone();
                async move {
                    if let Some(webhook_url) = payload.webhook_url.as_deref() {
                        let trimmed = webhook_url.trim();
                        if !trimmed.is_empty() {
                            if let Err(e) = validate_ssrf_url(trimmed).await {
                                return (
                                    StatusCode::BAD_REQUEST,
                                    Json(serde_json::json!({"status": "error", "message": format!("推送地址不合法: {}", e)}))
                                ).into_response();
                            }
                            let _ = dl.upsert_setting("webhook_url", trimmed).await;
                            let _ = dl.upsert_webhook(trimmed).await;
                        } else {
                            let _ = dl.upsert_setting("webhook_url", "").await;
                        }
                    }

                    if let Some(update_rate) = payload.update_rate {
                        let _ = dl.upsert_setting("update_rate", &update_rate.to_string()).await;
                    }


                    if let Some(decode_depth) = payload.decode_depth.as_deref() {
                        let _ = dl.upsert_setting("decode_depth", decode_depth).await;
                    }

                    if let Some(public_hub_url) = payload.public_hub_url.as_deref() {
                        let trimmed = public_hub_url.trim();
                        let _ = dl.upsert_setting("public_hub_url", trimmed).await;
                        if !trimmed.is_empty() {
                            let _ = tm.start(4000, None, Some(trimmed.to_string())).await;
                        }
                    }

                    if let Some(account_domain) = payload.account_domain.as_deref() {
                        let trimmed = account_domain.trim();
                        if !trimmed.is_empty() {
                            let _ = dl.upsert_setting("account_domain", trimmed).await;
                        }
                    }

                    if let Some(cloudflare_default_mode) = payload.cloudflare_default_mode.as_deref() {
                        let trimmed = cloudflare_default_mode.trim();
                        if !trimmed.is_empty() {
                            let _ = dl.upsert_setting("cloudflare_default_mode", trimmed).await;
                        }
                    }

                    if let Some(cloudflare_public_url) = payload.cloudflare_public_url.as_deref() {
                        let trimmed = cloudflare_public_url.trim();
                        if !trimmed.is_empty() {
                            let _ = dl.upsert_setting("cloudflare_public_url", trimmed).await;
                        }
                    }

                    if let Some(cloudflare_route_local_part) = payload.cloudflare_route_local_part.as_deref() {
                        let trimmed = cloudflare_route_local_part.trim();
                        if !trimmed.is_empty() {
                            let _ = dl.upsert_setting("cloudflare_route_local_part", trimmed).await;
                        }
                    }

                    if let Some(cloudflare_zone_domain) = payload.cloudflare_zone_domain.as_deref() {
                        let trimmed = cloudflare_zone_domain.trim();
                        if !trimmed.is_empty() {
                            let _ = dl.upsert_setting("cloudflare_zone_domain", trimmed).await;
                        }
                    }

                    if let Some(cloudflare_api_token) = payload.cloudflare_api_token.as_deref() {
                        let trimmed = cloudflare_api_token.trim();
                        if trimmed != "******" {
                            let _ = dl.upsert_setting("cloudflare_api_token", trimmed).await;
                        }
                    }

                    if let Some(cloudflare_zone_id) = payload.cloudflare_zone_id.as_deref() {
                        let trimmed = cloudflare_zone_id.trim();
                        if !trimmed.is_empty() {
                            let _ = dl.upsert_setting("cloudflare_zone_id", trimmed).await;
                        }
                    }

                    if let Some(cloudflare_account_id) = payload.cloudflare_account_id.as_deref() {
                        let trimmed = cloudflare_account_id.trim();
                        if !trimmed.is_empty() {
                            let _ = dl.upsert_setting("cloudflare_account_id", trimmed).await;
                        }
                    }


                    if let Some(cpa_url) = payload.cpa_url.as_deref() {
                        let trimmed = cpa_url.trim();
                        if !trimmed.is_empty() {
                            if let Err(e) = validate_ssrf_url(trimmed).await {
                                return (
                                    StatusCode::BAD_REQUEST,
                                    Json(serde_json::json!({"status": "error", "message": format!("CPA 接口地址不合法: {}", e)}))
                                ).into_response();
                            }
                            let _ = dl.upsert_setting("cpa_url", trimmed).await;
                        } else {
                            let _ = dl.upsert_setting("cpa_url", "").await;
                        }
                    }

                    if let Some(cpa_key) = payload.cpa_key.as_deref() {
                        let trimmed = cpa_key.trim();
                        if trimmed != "******" {
                            let _ = dl.upsert_setting("cpa_key", trimmed).await;
                        }
                    }

                    if let Some(sub2api_url) = payload.sub2api_url.as_deref() {
                        let trimmed = sub2api_url.trim();
                        if !trimmed.is_empty() {
                            if let Err(e) = validate_ssrf_url(trimmed).await {
                                return (
                                    StatusCode::BAD_REQUEST,
                                    Json(serde_json::json!({"status": "error", "message": format!("Sub2API 接口地址不合法: {}", e)}))
                                ).into_response();
                            }
                            let _ = dl.upsert_setting("sub2api_url", trimmed).await;
                        } else {
                            let _ = dl.upsert_setting("sub2api_url", "").await;
                        }
                    }

                    if let Some(sub2api_key) = payload.sub2api_key.as_deref() {
                        let trimmed = sub2api_key.trim();
                        if trimmed != "******" {
                            let _ = dl.upsert_setting("sub2api_key", trimmed).await;
                        }
                    }

                    if let Some(cpa_auth_json) = payload.cpa_auth_json.as_deref() {
                        let trimmed = cpa_auth_json.trim();
                        if trimmed != "******" {
                            let _ = dl.upsert_setting("cpa_auth_json", trimmed).await;
                        }
                    }

                    Json(serde_json::json!({"status": "success"})).into_response()
                }
            }
        }))
        .route("/api/cpa/oauth-url", get({
            move || {
                async move {
                    let pkce = openai::oauth::generate_pkce();
                    let state = openai::oauth::generate_state();
                    let url = format!(
                        "https://auth.openai.com/oauth/authorize?client_id={}&code_challenge={}&code_challenge_method=S256&codex_cli_simplified_flow=true&id_token_add_organizations=true&prompt=login&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&response_type=code&scope=openid+email+profile+offline_access&state={}",
                        openai::constants::OPENAI_CLIENT_ID,
                        pkce.code_challenge,
                        state
                    );
                    Json(serde_json::json!({"url": url, "code_verifier": pkce.code_verifier, "state": state}))
                }
            }
        }))
        .route("/api/cpa/exchange", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<HashMap<String, String>>| {
                let dl = dl.clone();
                async move {
                    let callback_url = match payload.get("callback_url") {
                        Some(v) => v,
                        None => return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"status": "error", "message": "缺少 callback_url"})))),
                    };
                    let code_verifier = match payload.get("code_verifier") {
                        Some(v) => v,
                        None => return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"status": "error", "message": "缺少 code_verifier"})))),
                    };
                    match openai::oauth::exchange_codex_code(callback_url, code_verifier).await {
                        Ok(auth_data) => {
                            let json_str = serde_json::to_string_pretty(&auth_data).unwrap();
                            let _ = dl.upsert_setting("cpa_auth_json", &json_str).await;

                            if env::var("WRITE_CODEX_AUTH_FILE")
                                .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
                                .unwrap_or(false)
                            {
                                if let Err(e) = std::fs::write("codex_auth.json", &json_str) {
                                    eprintln!("写入 codex_auth.json 失败: {e:?}");
                                }
                            }

                            Ok(Json(serde_json::json!({
                                "status": "success",
                                "email": auth_data.get_email()
                            })))
                        },
                        Err(e) => Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"status": "error", "message": e})))),
                    }
                }
            }
        }))
        .route("/api/oauth/register-url", get({
            move |Query(params): Query<HashMap<String, String>>| {
                async move {
                    let platform = params.get("platform").map(|s| s.as_str()).unwrap_or("cpa");
                    let redirect_encoded = if platform == "sub2api" {
                        "http%3A%2F%2Flocalhost%3A1456%2Fauth%2Fcallback"
                    } else {
                        "http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"
                    };

                    let pkce = openai::oauth::generate_pkce();
                    let state = openai::oauth::generate_state();
                    let url = format!(
                        "https://auth.openai.com/oauth/authorize?client_id={}&code_challenge={}&code_challenge_method=S256&codex_cli_simplified_flow=true&id_token_add_organizations=true&prompt=login&redirect_uri={}&response_type=code&scope=openid+email+profile+offline_access&state={}",
                        openai::constants::OPENAI_CLIENT_ID,
                        pkce.code_challenge,
                        redirect_encoded,
                        state
                    );
                    Json(serde_json::json!({
                        "url": url,
                        "code_verifier": pkce.code_verifier,
                        "state": state
                    }))
                }
            }
        }))
        .route("/api/oauth/register-exchange", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<serde_json::Value>| {
                let dl = dl.clone();
                async move {
                    let callback_url = match payload.get("callback_url").and_then(|v| v.as_str()) {
                        Some(v) => v,
                        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"status": "error", "message": "缺少 callback_url"}))).into_response(),
                    };
                    let code_verifier = match payload.get("code_verifier").and_then(|v| v.as_str()) {
                        Some(v) => v,
                        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"status": "error", "message": "缺少 code_verifier"}))).into_response(),
                    };
                    let platform = payload.get("platform").and_then(|v| v.as_str()).unwrap_or("cpa");
                    let redirect_uri = if platform == "sub2api" {
                        "http://localhost:1456/auth/callback"
                    } else {
                        "http://localhost:1455/auth/callback"
                    };

                    match openai::oauth::exchange_codex_code_with_redirect(callback_url, code_verifier, redirect_uri).await {
                        Ok(auth_data) => {
                            let email = auth_data.get_email().unwrap_or_else(|| {
                                format!("oauth_{}@openai.com", uuid::Uuid::new_v4().simple())
                            });
                            let password = format!("OAuth_{}", uuid::Uuid::new_v4().simple().to_string().chars().take(8).collect::<String>());

                            let account_id = match dl.create_generated_account(
                                "oauth_register",
                                &email,
                                &password,
                                "success",
                                Some("free"),
                                None
                            ).await {
                                Ok(id) => id,
                                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"status": "error", "message": format!("创建账号记录失败: {:?}", e)}))).into_response(),
                            };

                            let oauth_input = openai::oauth::OAuthCredentialInput {
                                email: &email,
                                access_token: Some(&auth_data.access_token),
                                refresh_token: Some(&auth_data.refresh_token),
                                id_token: Some(&auth_data.id_token),
                                workspace_id: None,
                                chatgpt_account_id: None,
                                chatgpt_user_id: None,
                                organization_id: None,
                                plan_type: None,
                                expires_in: Some(auth_data.expires_in as i64),
                                token_version: None,
                                stored_credentials: None,
                            };
                            let built = openai::oauth::build_oauth_credentials(oauth_input);

                            match dl.update_account_tokens(
                                &account_id,
                                Some(&built.access_token),
                                Some(&built.refresh_token),
                                None,
                                None,
                                None,
                                Some(&built.id_token),
                                Some(&built.chatgpt_account_id),
                                Some(&built.chatgpt_user_id),
                                Some(&built.organization_id),
                                Some(&built.plan_type),
                                Some(built.expires_in),
                                Some(built.token_version),
                                built.json.as_deref(),
                            ).await {
                                Ok(_) => Json(serde_json::json!({
                                    "status": "success",
                                    "email": email,
                                    "account_id": account_id
                                })).into_response(),
                                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"status": "error", "message": format!("更新令牌凭证失败: {:?}", e)}))).into_response(),
                            }
                        },
                        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"status": "error", "message": e}))).into_response()
                    }
                }
            }
        }))
        .route("/api/cpa/auth-status", get({
            let dl = Arc::clone(&data_lake);
            move || {
                let dl = dl.clone();
                async move {
                    match dl.get_setting("cpa_auth_json").await {
                        Ok(Some(json_str)) => {
                            if let Ok(auth_data) = serde_json::from_str::<openai::oauth::CodexAuthData>(&json_str) {
                                let email = auth_data.get_email().unwrap_or_else(|| "Codex Service".to_string());
                                Json(serde_json::json!({"status": "authenticated", "email": email}))
                            } else {
                                Json(serde_json::json!({"status": "invalid"}))
                            }
                        },
                        _ => Json(serde_json::json!({"status": "unauthenticated"}))
                    }
                }
            }
        }))
        .route("/api/cloudflare/automation/status", get({
            let manager = Arc::clone(&automation_manager);
            move || {
                let manager = manager.clone();
                async move { Json(manager.status()) }
            }
        }))
        .route("/api/cloudflare/automation/run", post({
            let manager = Arc::clone(&automation_manager);
            move |Json(payload): Json<CloudflareAutomationRunPayload>| {
                let manager = manager.clone();
                async move {
                    match manager.start(payload) {
                        Ok(_) => Json(serde_json::json!({"status": "started"})).into_response(),
                        Err(error) => (
                            StatusCode::CONFLICT,
                            Json(serde_json::json!({"status": "error", "message": error})),
                        )
                            .into_response(),
                    }
                }
            }
        }))
        .route("/debug/:name", get({
            let enabled = app_config.debug_assets_enabled;
            move |axum::extract::Path(name): axum::extract::Path<String>| async move {
                debug_asset(name, enabled).await
            }
        }))
        .route("/api/proxy/test", post(test_proxy_route))
        .layer(axum::middleware::from_fn_with_state(Arc::clone(&data_lake), auth_middleware))
        .fallback_service(
            ServeDir::new(web_dist)
                .append_index_html_on_directories(true)
        )
        .with_state(stream_hub)
}

#[derive(Deserialize)]
struct ProxyTestPayload {
    proxy_url: String,
}

// 测试代理服务器的联通性并获取延迟
async fn test_proxy_route(Json(payload): Json<ProxyTestPayload>) -> impl IntoResponse {
    let proxy_url = payload.proxy_url.trim();
    if proxy_url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": "代理地址不能为空"})),
        )
            .into_response();
    }

    let proxy = match reqwest::Proxy::all(proxy_url) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"status": "error", "message": format!("无效的代理地址格式: {}", e)})),
            )
                .into_response();
        }
    };

    let client = match reqwest::Client::builder()
        .proxy(proxy)
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"status": "error", "message": format!("无法构建测试请求客户端: {}", e)})),
            )
                .into_response();
        }
    };

    let start = std::time::Instant::now();
    // 使用 Cloudflare 作为网络可用性测试目标，也是注册环境的前置重要通道
    let target = "https://www.cloudflare.com";
    match client.get(target).send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis();
            let status = resp.status().as_u16();
            Json(serde_json::json!({
                "status": "success",
                "message": format!("联通测试成功！目标服务器响应状态码: {}", status),
                "latency_ms": latency,
            }))
            .into_response()
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": format!("联通测试失败: {}", e)
        }))
        .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        auth_session_token, constant_time_eq, hash_admin_password, is_intranet_ip,
        validate_admin_password, validate_admin_username, validate_legacy_auth_migration,
        verify_admin_password,
    };
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn session_token_is_derived_and_stable() {
        let token = auth_session_token("admin", "password-hash");
        assert_ne!(token, "password-hash");
        assert_eq!(token, auth_session_token("admin", "password-hash"));
    }

    #[test]
    fn secret_comparison_checks_content_and_length() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"different"));
    }

    #[test]
    fn admin_credential_rules_enforce_safe_bounds() {
        assert!(validate_admin_username("admin").is_ok());
        assert!(validate_admin_username("ab").is_err());
        assert!(validate_admin_password("twelve-chars!").is_ok());
        assert!(validate_admin_password("too-short").is_err());
    }

    #[tokio::test]
    async fn admin_password_is_hashed_and_verified() {
        let password = "correct-horse-battery-staple";
        let hash = hash_admin_password(password.to_string())
            .await
            .expect("password should hash");
        assert!(hash.starts_with("$argon2"));
        assert!(!hash.contains(password));
        assert!(verify_admin_password(hash.clone(), password.to_string()).await);
        assert!(!verify_admin_password(hash, "wrong-password-value".to_string()).await);
    }

    #[test]
    fn legacy_machine_secret_migration_is_fail_closed() {
        assert!(validate_legacy_auth_migration(None, None).is_ok());
        assert!(validate_legacy_auth_migration(Some("old-secret"), None).is_err());
        assert!(validate_legacy_auth_migration(Some("old-secret"), Some("wrong-secret")).is_err());
        assert!(validate_legacy_auth_migration(Some(" old-secret "), Some("old-secret")).is_ok());
    }

    #[test]
    fn blocks_private_and_loopback_addresses() {
        assert!(is_intranet_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_intranet_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_intranet_ip(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))));
        assert!(is_intranet_ip(IpAddr::V4(Ipv4Addr::new(198, 18, 0, 1))));
        assert!(is_intranet_ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))));
        assert!(!is_intranet_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }
}
