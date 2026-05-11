mod emails;

use crate::cloudflare_automation::{CloudflareAutomationManager, CloudflareAutomationRunPayload};
use crate::config::AppConfig;
use crate::db::DataLake;
use crate::stream::StreamHub;
use crate::{openai, stream, tunnel, workflow};
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
use std::path::{Component, Path as FsPath};
use std::sync::Arc;
use tower_http::services::ServeDir;

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
    auth_secret: Option<String>,
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

pub fn validate_ssrf_url(url_str: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(url_str).map_err(|e| format!("无效的 URL 格式: {}", e))?;

    let scheme = parsed.scheme().to_lowercase();
    if scheme != "https" {
        if scheme == "http" {
            if let Some(host) = parsed.host_str() {
                if host != "localhost" && host != "127.0.0.1" && host != "::1" {
                    return Err(
                        "非 HTTPS 协议仅允许在 localhost/127.0.0.1 环回地址下使用".to_string()
                    );
                }
            } else {
                return Err("仅支持 HTTPS 协议".to_string());
            }
        } else {
            return Err("仅支持 HTTPS 或本地 HTTP 协议".to_string());
        }
    }

    if let Some(host) = parsed.host_str() {
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            if is_intranet_ip(ip) {
                return Err("禁止使用内网 IP 地址进行外联".to_string());
            }
        } else {
            use std::net::ToSocketAddrs;
            let host_str = host.to_string();
            let is_intranet = std::thread::spawn(move || {
                if let Ok(addrs) = (host_str.as_str(), 80).to_socket_addrs() {
                    for addr in addrs {
                        if is_intranet_ip(addr.ip()) {
                            if addr.ip().is_loopback() {
                                continue;
                            }
                            return true;
                        }
                    }
                }
                false
            })
            .join()
            .unwrap_or(false);

            if is_intranet {
                return Err("DNS 解析结果包含内网 IP 地址，禁止外联".to_string());
            }
        }
    } else {
        return Err("无效的主机名".to_string());
    }

    Ok(parsed)
}

fn is_intranet_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() || ipv4.is_unspecified()
        }
        std::net::IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
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
            .body(axum::body::Body::from(format!(
                r#"{{"status":"error","message":"{}"}}"#,
                message
            )))
            .unwrap_or_else(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, "Response build error").into_response()
            });
    }

    let login_html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>PhantomDrop - Auth</title>
    <style>
        body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;background:#f8fafc;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}
        .card{background:#fff;padding:30px;border-radius:12px;box-shadow:0 4px 20px rgba(0,0,0,.05);width:320px;text-align:center;border:1px solid #e2e8f0}
        h2{margin-top:0;color:#0f172a;font-size:1.5rem}p{color:#64748b;font-size:.875rem;margin-bottom:24px}
        input{width:100%;padding:10px;border:1px solid #cbd5e1;border-radius:6px;box-sizing:border-box;margin-bottom:16px;font-size:14px;outline:none}
        input:focus{border-color:#2563eb}button{width:100%;padding:10px;background:#2563eb;color:white;border:0;border-radius:6px;font-size:14px;font-weight:700;cursor:pointer}
        button:hover{background:#1d4ed8}.error{color:#ef4444;font-size:12px;margin-top:10px;display:none}
    </style>
</head>
<body>
    <div class="card">
        <h2>System Auth</h2>
        <p>Enter auth_secret to continue.</p>
        <input type="password" id="secret" placeholder="auth_secret" onkeydown="if(event.key==='Enter')login()">
        <button onclick="login()">Login</button>
        <div id="error" class="error">Invalid secret or auth is not configured.</div>
    </div>
    <script>
        function login(){
            var val=document.getElementById('secret').value;
            if(!val)return;
            var secure=location.protocol==='https:'?'; Secure':'';
            document.cookie='phantom_auth_token='+encodeURIComponent(val)+'; path=/; max-age=31536000; SameSite=Lax'+secure;
            window.location.reload();
        }
        if(document.cookie.indexOf('phantom_auth_token')>-1){document.getElementById('error').style.display='block';}
    </script>
</body>
</html>"#;

    axum::response::Response::builder()
        .status(status)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(login_html))
        .unwrap_or_else(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Response build error").into_response()
        })
}

async fn get_auth_secret(dl: &DataLake) -> Option<String> {
    if let Ok(Some(secret)) = dl.get_setting("auth_secret").await {
        let trimmed = secret.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    if let Ok(secret) = std::env::var("HUB_SECRET") {
        let trimmed = secret.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

async fn auth_middleware(
    axum::extract::State(data_lake): axum::extract::State<Arc<DataLake>>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let path = req.uri().path();

    if path == "/health" || path == "/ingest" {
        return Ok(next.run(req).await);
    }

    let is_protected = path.starts_with("/api/")
        || path == "/stream"
        || path == "/stream/"
        || path.starts_with("/console");

    if is_protected {
        let wants_html = !(path.starts_with("/api/") || path.starts_with("/stream"));
        let Some(expected_secret) = get_auth_secret(&data_lake).await else {
            return Ok(auth_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "认证密钥未配置，管理接口已锁定",
                wants_html,
            ));
        };

        let mut provided_secret = None;

        if let Some(auth_val) = req
            .headers()
            .get("authorization")
            .and_then(|h| h.to_str().ok())
        {
            if auth_val.to_lowercase().starts_with("bearer ") {
                provided_secret = Some(auth_val[7..].trim().to_string());
            }
        }

        if provided_secret.is_none() {
            if let Some(token_val) = req
                .headers()
                .get("x-auth-token")
                .and_then(|h| h.to_str().ok())
            {
                provided_secret = Some(token_val.trim().to_string());
            }
        }

        if provided_secret.is_none() {
            if let Some(cookie_val) = req.headers().get("cookie").and_then(|h| h.to_str().ok()) {
                provided_secret = extract_cookie_token(cookie_val);
            }
        }

        let authenticated = match provided_secret {
            Some(secret) => secret == expected_secret,
            None => false,
        };

        if !authenticated {
            return Ok(auth_error_response(
                StatusCode::UNAUTHORIZED,
                "未授权，请输入正确的接口密钥",
                wants_html,
            ));
        }
    }

    Ok(next.run(req).await)
}

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
        auth_secret: mask_credential(map.get("auth_secret").cloned().filter(|v| !v.is_empty())),
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
                            eprintln!("读取配置失败: {:?}", e);
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
                            eprintln!("读取统计失败: {:?}", e);
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
                        eprintln!(" - 原因: {}", message);
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
                            eprintln!("保存工作流定义失败: {:?}", e);
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
                            eprintln!("删除工作流定义失败: {:?}", e);
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
                            eprintln!("读取工作流执行记录失败: {:?}", e);
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
                            eprintln!("读取工作流步骤失败: {:?}", e);
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
                            eprintln!("停止工作流失败: {:?}", e);
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
                        Ok(accounts) => Json(accounts).into_response(),
                        Err(e) => {
                            eprintln!("读取生成账号失败: {:?}", e);
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
                            eprintln!("导出生成账号失败: {:?}", e);
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
                            eprintln!("OTP 轮询失败: {:?}", e);
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
                            "items": items,
                            "limit": limit,
                            "offset": offset,
                            "total": total,
                        })).into_response(),
                        (Err(e), _) | (_, Err(e)) => {
                            eprintln!("读取全局账号列表失败: {:?}", e);
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
                            eprintln!("读取全部账号 ID 失败: {:?}", e);
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
                            eprintln!("读取账号 Token 失败: {:?}", e);
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

                    match dl.update_account_tokens(
                        &id,
                        access_token,
                        refresh_token,
                        session_token,
                        device_id,
                        workspace_id,
                        Some(oauth_credentials.id_token.as_str()),
                        Some(oauth_credentials.chatgpt_account_id.as_str()),
                        Some(oauth_credentials.chatgpt_user_id.as_str()),
                        Some(oauth_credentials.organization_id.as_str()),
                        Some(oauth_credentials.plan_type.as_str()),
                        Some(oauth_credentials.expires_in),
                        Some(oauth_credentials.token_version),
                        oauth_credentials.json.as_deref(),
                    ).await {
                        Ok(_) => Json(serde_json::json!({"status": "success"})).into_response(),
                        Err(e) => {
                            eprintln!("更新账号 Token 失败: {:?}", e);
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
                            eprintln!("删除账号失败: {:?}", e);
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
                            eprintln!("检查账号状态失败: {:?}", e);
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
                                Err(e) => format!("Error: {}", e),
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
                            eprintln!("批量删除账号失败: {:?}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "批量删除账号失败"}))
                            ).into_response()
                        }
                    }
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
                            eprintln!("清理失败账号失败: {:?}", e);
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

                    if let Err(e) = validate_ssrf_url(&cpa_url) {
                        return (StatusCode::BAD_REQUEST, format!("CPA 接口地址安全校验失败: {}", e)).into_response();
                    }

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

                    let client = reqwest::Client::new();
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
                                     eprintln!("CPA 上传失败 ({}): {}", id, e);
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

                    if let Err(e) = validate_ssrf_url(&sub2api_url) {
                        return (StatusCode::BAD_REQUEST, format!("Sub2API 接口地址安全校验失败: {}", e)).into_response();
                    }
                    let sub2api_key = settings.get("sub2api_key").cloned().unwrap_or_default();

                    let client = reqwest::Client::new();
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
                                     eprintln!("Sub2API 上传失败 ({}): {}", id, e);
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

                    if params.get("format").map(|s| s.as_str()) == Some("oauth") {
                        let oauth_json = crate::exporter::AccountExporter::export_to_oauth_json(&results);
                        Json(oauth_json).into_response()
                    } else {
                        Json(results).into_response()
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
                                eprintln!("保存公网地址失败: {:?}", e);
                            }
                            if let Err(e) = dl.upsert_setting("public_hub_port", &payload.port.to_string()).await {
                                eprintln!("保存公网端口失败: {:?}", e);
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
                        eprintln!("清理公网地址失败: {:?}", e);
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
                            if let Err(e) = validate_ssrf_url(trimmed) {
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

                    if let Some(auth_secret) = payload.auth_secret.as_deref() {
                        let trimmed = auth_secret.trim();
                        if trimmed != "******" {
                            let _ = dl.upsert_setting("auth_secret", trimmed).await;
                        }
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
                            if let Err(e) = validate_ssrf_url(trimmed) {
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
                            if let Err(e) = validate_ssrf_url(trimmed) {
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
                                    eprintln!("写入 codex_auth.json 失败: {:?}", e);
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
        .layer(axum::middleware::from_fn_with_state(Arc::clone(&data_lake), auth_middleware))
        .fallback_service(
            ServeDir::new(web_dist)
                .append_index_html_on_directories(true)
        )
        .with_state(stream_hub)
}
