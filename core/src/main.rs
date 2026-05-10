mod cloudflare_automation;
mod config;
mod db;
mod exporter;
mod openai;
mod parser;
mod register;
mod routes;
mod stream;
mod tunnel;
mod uploader;
mod workflow;

use crate::cloudflare_automation::CloudflareAutomationManager;
use crate::config::{AppConfig, validate_hub_secret_for_environment};
use crate::db::DataLake;
use crate::stream::StreamHub;
use std::env;
use std::sync::Arc;

/**
 * 幻影中台 (PhantomDrop-Hub) - 核心中枢
 * 职责：汇聚边缘邮件，提供类型安全控制端，驱动 AI 解析流
 */

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
    let app_config = AppConfig::from_env().unwrap_or_else(|error| {
        eprintln!("配置错误: {error}");
        std::process::exit(1);
    });

    // 1. 初始化数据湖 (SQLite)
    let database_url = std::env::var("PHANTOM_DB_URL")
        .unwrap_or_else(|_| "sqlite://phantom_core.db?mode=rwc".to_string());
    let data_lake = DataLake::new(&database_url).await;
    let saved_settings = data_lake.list_settings().await.unwrap_or_default();
    if let Err(error) = validate_hub_secret_for_environment(
        &app_config,
        env::var("HUB_SECRET")
            .ok()
            .as_deref()
            .or_else(|| saved_settings.get("auth_secret").map(String::as_str)),
    ) {
        eprintln!("安全配置错误: {error}");
        std::process::exit(1);
    }
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
    let app = routes::build_router(routes::RouterContext {
        data_lake: Arc::clone(&data_lake),
        stream_hub: Arc::clone(&stream_hub),
        workflow_engine: Arc::clone(&workflow_engine),
        tunnel_manager: Arc::clone(&tunnel_manager),
        automation_manager: Arc::clone(&automation_manager),
        app_config: app_config.clone(),
        web_dist,
    })
    .layer(app_config.cors_layer());

    // 4. 开启监听
    let listener = tokio::net::TcpListener::bind(app_config.bind_addr).await.unwrap();
    println!("⚡ 监听中枢已就绪: http://{}", app_config.bind_addr);
    axum::serve(listener, app).await.unwrap();
}

