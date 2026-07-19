use crate::db::DataLake;
use crate::parser::{NeuralParser, ParseDepth};
use crate::stream::{StreamHub, StreamPayload};
use axum::extract::{Path, Query};
use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
#[allow(dead_code)]
struct EmailIngestPayload {
    meta: EmailMeta,
    content: EmailContent,
    #[serde(default)]
    attachments: Vec<AttachmentMeta>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct AttachmentMeta {
    #[serde(default)]
    filename: Option<String>,
    #[serde(alias = "mimeType", default)]
    mime_type: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct EmailMeta {
    from: String,
    to: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    date: Option<String>,
}

#[derive(Deserialize)]
struct EmailContent {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    html: Option<String>,
}

#[derive(Deserialize, Default)]
struct EmailQuery {
    q: Option<String>,
    limit: Option<i64>,
    page: Option<i64>,
    page_size: Option<i64>,
    archived: Option<bool>,
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

pub fn start_webhook_worker(data_lake: Arc<DataLake>) {
    tokio::spawn(async move {
        if let Err(error) = data_lake.recover_webhook_outbox().await {
            eprintln!("Webhook Outbox 恢复失败: {error}");
        }
        let mut last_recovery = std::time::Instant::now();
        loop {
            if last_recovery.elapsed() >= std::time::Duration::from_secs(60) {
                if let Err(error) = data_lake.recover_webhook_outbox().await {
                    eprintln!("Webhook Outbox 定期恢复失败: {error}");
                }
                last_recovery = std::time::Instant::now();
            }
            match data_lake.lease_webhook_delivery().await {
                Ok(Some(delivery)) => {
                    let result = deliver_webhook(&delivery.webhook_url, &delivery.payload).await;
                    let update = match result {
                        Ok(()) => data_lake.complete_webhook_delivery(&delivery.id).await,
                        Err(error) => {
                            eprintln!(
                                "Webhook 投递失败: url={}, attempt={}, error={error}",
                                delivery.webhook_url,
                                delivery.attempts + 1
                            );
                            data_lake
                                .fail_webhook_delivery(&delivery.id, delivery.attempts, &error)
                                .await
                        }
                    };
                    if let Err(error) = update {
                        eprintln!("Webhook Outbox 状态更新失败: {error}");
                    }
                }
                Ok(None) => tokio::time::sleep(std::time::Duration::from_millis(500)).await,
                Err(error) => {
                    eprintln!("Webhook Outbox 读取失败: {error}");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
    });
}

async fn deliver_webhook(url: &str, payload: &str) -> Result<(), String> {
    let (validated_url, client) = crate::routes::build_ssrf_safe_client(url).await?;
    let payload: serde_json::Value =
        serde_json::from_str(payload).map_err(|error| format!("Webhook payload 无效: {error}"))?;
    let response = client
        .post(validated_url)
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("Webhook 请求失败: {error}"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("Webhook 返回状态 {}", response.status()))
    }
}
pub fn routes(data_lake: Arc<DataLake>, stream_hub: Arc<StreamHub>) -> Router<Arc<StreamHub>> {
    Router::new()
        .route("/api/emails", get({
            let dl = Arc::clone(&data_lake);
            move |Query(query): Query<EmailQuery>| {
                let dl = dl.clone();
                async move {
                    let limit = query.limit.unwrap_or(100);
                    match dl.get_emails(limit, query.q.as_deref(), query.archived).await {
                        Ok(emails) => Json(serde_json::json!(emails)).into_response(),
                        Err(e) => {
                            eprintln!("Failed to fetch emails: {e:?}");
                            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch emails").into_response()
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
                            eprintln!("分页读取邮件失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "分页读取邮件失败"})),
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
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"status": "error", "message": "邮件不存在"})),
                        ).into_response(),
                        Err(e) => {
                            eprintln!("读取邮件详情失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "读取邮件详情失败"})),
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
                            Json(serde_json::json!({"status": "error", "message": "邮件不存在"})),
                        ).into_response(),
                        Err(e) => {
                            eprintln!("归档邮件失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "归档邮件失败"})),
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
                            eprintln!("批量归档邮件失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "批量归档邮件失败"})),
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
                            Json(serde_json::json!({"status": "error", "message": "邮件不存在"})),
                        ).into_response(),
                        Err(e) => {
                            eprintln!("删除邮件失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "删除邮件失败"})),
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
                            eprintln!("批量删除邮件失败: {e:?}");
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"status": "error", "message": "批量删除邮件失败"})),
                            ).into_response()
                        }
                    }
                }
            }
        }))
        .route("/ingest", post({
            let dl = Arc::clone(&data_lake);
            let hub = Arc::clone(&stream_hub);
            move |headers: HeaderMap, Json(payload): Json<EmailIngestPayload>| {
                let dl = dl.clone();
                let hub = hub.clone();
                async move {
                    let expected_secret = std::env::var("HUB_SECRET")
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                    if expected_secret.is_empty() {
                        eprintln!("邮件接入未启用：未配置 HUB_SECRET");
                        return (StatusCode::SERVICE_UNAVAILABLE, "邮件接入未启用").into_response();
                    }

                    let provided_secret = headers
                        .get("x-hub-secret")
                        .and_then(|val| val.to_str().ok())
                        .unwrap_or("")
                        .trim();
                    if !super::constant_time_eq(provided_secret.as_bytes(), expected_secret.as_bytes()) {
                        eprintln!("安全拦截：未授权的访问请求(Secret不匹配)");
                        return (StatusCode::UNAUTHORIZED, "安全验证失败").into_response();
                    }

                    let id = Uuid::new_v4().to_string();
                    let text = payload.content.text.as_deref().unwrap_or("");
                    let html = payload.content.html.as_deref().unwrap_or("");
                    let from = payload.meta.from.clone();
                    let to = payload.meta.to.clone();
                    let subject = payload.meta.subject.as_deref().unwrap_or("无主题").to_string();
                    let decode_depth = dl.get_setting("decode_depth").await.ok().flatten();
                    let mut parsed = NeuralParser::parse_all(text, html, ParseDepth::from_setting(decode_depth.as_deref()));
                    if NeuralParser::is_openai_sender(&from) {
                        parsed.code = NeuralParser::extract_openai_otp(text, html);
                    }

                    let webhook_payload = serde_json::json!({
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
                    let webhook_payload = webhook_payload.to_string();

                    if let Err(error) = dl
                        .record_email(
                            &id,
                            &from,
                            &to,
                            &subject,
                            text,
                            html,
                            parsed.code.as_deref(),
                            parsed.link.as_deref(),
                            parsed.custom_text.as_deref(),
                            Some(&webhook_payload),
                        )
                        .await
                    {
                        eprintln!("邮件与回调任务入库失败: id={id}, error={error:?}");
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "邮件入库失败，请稍后重试",
                        )
                            .into_response();
                    }

                    eprintln!(
                        "邮件已入库: id={id}, from={from}, to={to}, subject={subject}, code={}",
                        parsed.code.as_deref().unwrap_or("")
                    );
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
                    (StatusCode::OK, "邮件已注入").into_response()
                }
            }
        }))
}
