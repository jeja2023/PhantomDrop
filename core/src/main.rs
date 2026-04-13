mod cloudflare_automation;
mod db;
mod openai;
mod parser;
mod register;
mod stream;
mod tunnel;
mod uploader;
mod workflow;

use crate::cloudflare_automation::{CloudflareAutomationManager, CloudflareAutomationRunPayload};
use crate::db::DataLake;
use crate::parser::{NeuralParser, ParseDepth};
use crate::stream::{StreamHub, StreamPayload};
use axum::extract::{Path, Query};
use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::{Html, IntoResponse},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;

/**
 * 幻影中台 (PhantomDrop-Hub) - 核心中枢
 * 职责：汇聚边缘邮件，提供类型安全控制端，驱动 AI 解析流
 */

#[derive(Deserialize)]
#[allow(dead_code)]
struct EmailIngestPayload {
    meta: EmailMeta,
    content: EmailContent,
    // 其他字段可选，防止前端多传导致的 Deserialize 失败
    #[serde(default)]
    attachments: Vec<AttachmentMeta>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AttachmentMeta {
    filename: Option<String>,
    #[serde(alias = "mimeType")]
    mime_type: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct EmailMeta {
    from: String,
    to: String,
    subject: Option<String>,
    date: Option<String>,
}

#[derive(Deserialize)]
struct EmailContent {
    text: Option<String>,
    html: Option<String>,
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
}

#[derive(Deserialize, Default)]
struct EmailQuery {
    q: Option<String>,
    limit: Option<i64>,
    page: Option<i64>,
    page_size: Option<i64>,
    archived: Option<bool>,
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
struct ArchiveEmailPayload {
    archived: bool,
}

#[derive(Deserialize)]
struct EmailBatchPayload {
    ids: Vec<String>,
    archived: Option<bool>,
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

fn settings_from_map(map: HashMap<String, String>) -> SettingsPayload {
    SettingsPayload {
        webhook_url: map
            .get("webhook_url")
            .cloned()
            .filter(|value| !value.is_empty()),
        update_rate: map
            .get("update_rate")
            .and_then(|value| value.parse::<u64>().ok()),
        auth_secret: map
            .get("auth_secret")
            .cloned()
            .filter(|value| !value.is_empty()),
        decode_depth: map
            .get("decode_depth")
            .cloned()
            .filter(|value| !value.is_empty()),
        public_hub_url: map
            .get("public_hub_url")
            .cloned()
            .filter(|value| !value.is_empty()),
        account_domain: map
            .get("account_domain")
            .cloned()
            .filter(|value| !value.is_empty()),
        cloudflare_default_mode: map
            .get("cloudflare_default_mode")
            .cloned()
            .filter(|value| !value.is_empty()),
        cloudflare_public_url: map
            .get("cloudflare_public_url")
            .cloned()
            .filter(|value| !value.is_empty()),
        cloudflare_route_local_part: map
            .get("cloudflare_route_local_part")
            .cloned()
            .filter(|value| !value.is_empty()),
        cloudflare_zone_domain: map
            .get("cloudflare_zone_domain")
            .cloned()
            .filter(|value| !value.is_empty()),
        cloudflare_api_token: map
            .get("cloudflare_api_token")
            .cloned()
            .filter(|value| !value.is_empty()),
        cloudflare_zone_id: map
            .get("cloudflare_zone_id")
            .cloned()
            .filter(|value| !value.is_empty()),
        cloudflare_account_id: map
            .get("cloudflare_account_id")
            .cloned()
            .filter(|value| !value.is_empty()),
    }
}

fn detect_project_root() -> std::path::PathBuf {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if current_dir.join("setup-cloudflare-mail.ps1").exists() {
        return current_dir;
    }

    if current_dir
        .parent()
        .map(|parent| parent.join("setup-cloudflare-mail.ps1").exists())
        .unwrap_or(false)
    {
        return current_dir.parent().unwrap().to_path_buf();
    }

    current_dir
}

#[tokio::main]
async fn main() {
    println!("🌌 幻影中枢 (PhantomDrop-Hub) 正在启动...");

    // 1. 初始化数据湖 (SQLite)
    let database_url = std::env::var("PHANTOM_DB_URL")
        .unwrap_or_else(|_| "sqlite://phantom_core.db?mode=rwc".to_string());
    let data_lake = DataLake::new(&database_url).await;
    let saved_settings = data_lake.list_settings().await.unwrap_or_default();
    let project_root = detect_project_root();

    // 2. 初始化实时流枢纽 (SSE)
    let stream_hub = StreamHub::new();

    // 3. 初始化自动化工作流引擎
    let workflow_engine = Arc::new(workflow::WorkflowEngine::new(
        Arc::clone(&stream_hub),
        Arc::clone(&data_lake),
    ));
    workflow_engine.ensure_builtin_definitions().await;

    // 4. 初始化内网穿透管理器
    let tunnel_manager = Arc::new(tunnel::TunnelManager::new());
    tunnel_manager.restore(
        saved_settings
            .get("public_hub_port")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(9010),
        saved_settings.get("public_hub_url").cloned(),
    );

    let automation_manager = Arc::new(CloudflareAutomationManager::new(
        project_root,
        Arc::clone(&stream_hub),
    ));

    // 5. 准备前端静态文件服务
    // 环境优先：支持外部指定前端目录，默认为当前目录下的 web 文件夹
    let web_dist = std::env::var("WEB_DIST").unwrap_or_else(|_| "web".to_string());
    println!("🌐 静态资源目录: {}", web_dist);

    // 6. 构建全站 API 网关
    let app = Router::new()
        .route("/console", get(console_index))
        .route("/console/", get(console_index))
        .route("/console/style.css", get(console_style))
        .route("/console/app.js", get(console_script))
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .route("/api/emails", get({
            let dl = Arc::clone(&data_lake);
            move |Query(query): Query<EmailQuery>| {
                let dl = dl.clone();
                async move {
                    let limit = query.limit.unwrap_or(100);
                    match dl.get_emails(limit, query.q.as_deref(), query.archived).await {
                        Ok(emails) => Json(serde_json::json!(emails)).into_response(),
                        Err(e) => {
                            eprintln!("Failed to fetch emails: {:?}", e);
                            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch emails").into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/emails/query", get({
            let dl = Arc::clone(&data_lake);
            move |Query(query): Query<EmailQuery>| {
                let dl = dl.clone();
                async move {
                    let page = query.page.unwrap_or(1);
                    let page_size = query.page_size.unwrap_or(20);
                    match dl.get_emails_page(page, page_size, query.q.as_deref(), query.archived).await {
                        Ok(result) => Json(result).into_response(),
                        Err(e) => {
                            eprintln!("分页读取邮件失败: {:?}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "分页读取邮件失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/emails/:id", get({
            let dl = Arc::clone(&data_lake);
            move |Path(id): Path<String>| {
                let dl = dl.clone();
                async move {
                    match dl.get_email_detail(&id).await {
                        Ok(Some(email)) => Json(email).into_response(),
                        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"status": "error", "message": "邮件不存在"}))).into_response(),
                        Err(e) => {
                            eprintln!("读取邮件详情失败: {:?}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取邮件详情失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/emails/:id/archive", post({
            let dl = Arc::clone(&data_lake);
            move |Path(id): Path<String>, Json(payload): Json<ArchiveEmailPayload>| {
                let dl = dl.clone();
                async move {
                    match dl.set_email_archived(&id, payload.archived).await {
                        Ok(count) if count > 0 => Json(serde_json::json!({"status": "success"})).into_response(),
                        Ok(_) => (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"status": "error", "message": "邮件不存在"}))
                        ).into_response(),
                        Err(e) => {
                            eprintln!("归档邮件失败: {:?}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "归档邮件失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/emails/batch/archive", post({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<EmailBatchPayload>| {
                let dl = dl.clone();
                async move {
                    let archived = payload.archived.unwrap_or(true);
                    match dl.set_emails_archived(&payload.ids, archived).await {
                        Ok(count) => Json(serde_json::json!({"status": "success", "updated": count})).into_response(),
                        Err(e) => {
                            eprintln!("批量归档邮件失败: {:?}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "批量归档邮件失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/emails/:id", delete({
            let dl = Arc::clone(&data_lake);
            move |Path(id): Path<String>| {
                let dl = dl.clone();
                async move {
                    match dl.delete_email(&id).await {
                        Ok(count) if count > 0 => Json(serde_json::json!({"status": "success"})).into_response(),
                        Ok(_) => (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"status": "error", "message": "邮件不存在"}))
                        ).into_response(),
                        Err(e) => {
                            eprintln!("删除邮件失败: {:?}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "删除邮件失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/api/emails/batch", delete({
            let dl = Arc::clone(&data_lake);
            move |Json(payload): Json<EmailBatchPayload>| {
                let dl = dl.clone();
                async move {
                    match dl.delete_emails(&payload.ids).await {
                        Ok(count) => Json(serde_json::json!({"status": "success", "deleted": count})).into_response(),
                        Err(e) => {
                            eprintln!("批量删除邮件失败: {:?}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "批量删除邮件失败"}))
                            ).into_response()
                        }
                    }
                }
            }
        }))
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
                            let mut csv = String::from("address,password,status,created_at\n");
                            for account in accounts {
                                let line = format!(
                                    "\"{}\",\"{}\",\"{}\",\"{}\"\n",
                                    account.address.replace('"', "\"\""),
                                    account.password.replace('"', "\"\""),
                                    account.status.replace('"', "\"\""),
                                    chrono::DateTime::from_timestamp(account.created_at, 0)
                                        .map(|dt| dt.naive_utc().to_string())
                                        .unwrap_or_else(|| account.created_at.to_string()),
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

                    match dl.list_all_accounts(limit, offset).await {
                        Ok(accounts) => Json(serde_json::json!({
                            "items": accounts,
                            "limit": limit,
                            "offset": offset,
                        })).into_response(),
                        Err(e) => {
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
        .route("/api/accounts/:id/tokens", post({
            let dl = Arc::clone(&data_lake);
            move |Path(id): Path<String>, Json(payload): Json<HashMap<String, String>>| {
                let dl = dl.clone();
                async move {
                    let access_token = payload.get("access_token").map(|s| s.as_str());
                    let refresh_token = payload.get("refresh_token").map(|s| s.as_str());
                    let session_token = payload.get("session_token").map(|s| s.as_str());
                    let device_id = payload.get("device_id").map(|s| s.as_str());
                    let workspace_id = payload.get("workspace_id").map(|s| s.as_str());

                    match dl.update_account_tokens(
                        &id, access_token, refresh_token, session_token, device_id, workspace_id
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
                            let _ = dl.upsert_setting("webhook_url", trimmed).await;
                            let _ = dl.upsert_webhook(trimmed).await;
                        }
                    }

                    if let Some(update_rate) = payload.update_rate {
                        let _ = dl.upsert_setting("update_rate", &update_rate.to_string()).await;
                    }

                    if let Some(auth_secret) = payload.auth_secret.as_deref() {
                        let _ = dl.upsert_setting("auth_secret", auth_secret).await;
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
                        if !trimmed.is_empty() {
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

                    Json(serde_json::json!({"status": "success"}))
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
        .route("/debug/:name", get(|axum::extract::Path(name): axum::extract::Path<String>| async move {
            let path = std::path::Path::new("./data").join(name);
            if !path.exists() || !path.is_file() {
                return (axum::http::StatusCode::NOT_FOUND, "Screenshot not found").into_response();
            }
            
            let content = match std::fs::read(&path) {
                Ok(c) => c,
                Err(_) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "Read error").into_response(),
            };

            axum::response::Response::builder()
                .header("Content-Type", "image/png")
                .body(axum::body::Body::from(content))
                .unwrap()
        }))
        .fallback_service(
            ServeDir::new(web_dist)
                .append_index_html_on_directories(true)
        )
        .route("/ingest", post({
            let dl = Arc::clone(&data_lake);
            let hub = Arc::clone(&stream_hub);
            move |headers: HeaderMap, Json(payload): Json<EmailIngestPayload>| {
                let dl = dl.clone();
                let hub = hub.clone();
                async move {
                    // 1. 安全层: 令牌校验逻辑
                    let expected_secret = match env::var("HUB_SECRET") {
                        Ok(value) if !value.trim().is_empty() => value.trim().to_string(),
                        _ => dl.get_setting("auth_secret").await.unwrap_or(None).unwrap_or_default().trim().to_string(),
                    };
                    if !expected_secret.is_empty() {
                        let provided_secret = headers
                            .get("X-Hub-Secret")
                            .and_then(|val| val.to_str().ok())
                            .unwrap_or("")
                            .trim();
                        if provided_secret != expected_secret {
                            eprintln!("安全拦截：未授权的访问请求(Secret不匹配)");
                            return (StatusCode::UNAUTHORIZED, "安全验证失败").into_response();
                        }
                    }

                    let id = Uuid::new_v4().to_string();
                    let text = payload.content.text.as_deref().unwrap_or("");
                    let html = payload.content.html.as_deref().unwrap_or("");
                    let from = payload.meta.from.clone();
                    let to = payload.meta.to.clone();
                    let subject = payload.meta.subject.as_deref().unwrap_or("无主题").to_string();

                    // 2. 环境智能纠错与多特征提取 (Neural Parser)
                    let decode_depth = dl
                        .get_setting("decode_depth")
                        .await
                        .ok()
                        .flatten();
                    let parsed = NeuralParser::parse_all(text, html, ParseDepth::from_setting(decode_depth.as_deref()));
                    
                    if let Err(e) = dl.record_email(
                        &id,
                        &from,
                        &to,
                        &subject,
                        text,
                        html,
                        parsed.code.as_deref(),
                        parsed.link.as_deref(),
                        parsed.custom_text.as_deref()
                    ).await {
                        eprintln!("入库失败: {:?}", e);
                    } else {
                        // 3. 实时广播到前端 (秒级推送)
                        hub.broadcast(StreamPayload {
                            id: id.clone(),
                            event_type: "new_email".into(),
                            data: serde_json::json!({
                                "id": id,
                                "from": from,
                                "to": to,
                                "subject": subject,
                                "code": parsed.code,
                                "link": parsed.link,
                                "custom_text": parsed.custom_text,
                            }),
                        });

                        // 4. 外部 Webhook 推送 (异步转发给指纹系统)
                        let dl_for_hook = dl.clone();
                        let payload_for_hook = serde_json::json!({
                            "id": id,
                            "type": "EMAIL_INGEST_READY",
                            "data": {
                                "from": from,
                                "to": to,
                                "subject": subject,
                                "code": parsed.code,
                                "link": parsed.link,
                            }
                        });

                        tokio::spawn(async move {
                            if let Ok(hooks) = dl_for_hook.get_active_webhooks().await {
                                let client = reqwest::Client::new();
                                for (url, _) in hooks {
                                    let _ = client.post(&url)
                                        .json(&payload_for_hook)
                                        .timeout(std::time::Duration::from_secs(5))
                                        .send()
                                        .await;
                                }
                            }
                        });
                    }
                    (StatusCode::OK, "邮件已注入").into_response()
                }
            }
        }))
        .with_state(stream_hub)
        .layer(CorsLayer::permissive());

    // 4. 开启监听
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9010").await.unwrap();
    println!("⚡ 监听中枢已就绪: http://0.0.0.0:9010");
    axum::serve(listener, app).await.unwrap();
}
