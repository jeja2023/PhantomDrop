use crate::db::DataLake;
use crate::openai::checker::check_account_status;
use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

static RATE_LIMITERS: OnceLock<Mutex<HashMap<String, TokenBucket>>> = OnceLock::new();

fn resolve_gateway_pool(configured_keys: &str, provided_key: &str) -> Option<String> {
    configured_keys.split(',').find_map(|entry| {
        let (key, pool) = entry.trim().split_once('=')?;
        if constant_time_eq(key.trim().as_bytes(), provided_key.as_bytes()) {
            let pool = pool.trim();
            Some(if pool.is_empty() {
                "default".to_string()
            } else {
                pool.to_string()
            })
        } else {
            None
        }
    })
}

fn constant_time_eq(expected: &[u8], provided: &[u8]) -> bool {
    let mut difference = expected.len() ^ provided.len();
    let length = expected.len().max(provided.len());
    for index in 0..length {
        let left = expected.get(index).copied().unwrap_or_default();
        let right = provided.get(index).copied().unwrap_or_default();
        difference |= usize::from(left ^ right);
    }
    difference == 0
}

fn error_response(status: StatusCode, message: &str) -> Response<Body> {
    let body = serde_json::json!({"error": {"message": message}}).to_string();
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::from("internal response error")))
}
/// 动态令牌桶限流判定
fn check_rate_limit(api_key: &str, max_tokens: f64, refill_rate: f64) -> bool {
    let limiters = RATE_LIMITERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = limiters.lock().unwrap();
    if map.len() >= 10_000 && !map.contains_key(api_key) {
        map.clear();
    }
    let bucket = map
        .entry(api_key.to_string())
        .or_insert_with(|| TokenBucket {
            tokens: max_tokens,
            last_refill: Instant::now(),
        });

    let now = Instant::now();
    let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
    bucket.last_refill = now;

    bucket.tokens = (bucket.tokens + elapsed * refill_rate).min(max_tokens);

    if bucket.tokens >= 1.0 {
        bucket.tokens -= 1.0;
        true
    } else {
        false
    }
}

/// 自给自足型 API 密钥与高可用反向代理网关。
/// 接收客户端的SK密匙或标准SK，自动进行分池、负载均衡、智能冷却、以及全自动的主动自愈与令牌刷新。
pub async fn chat_completions_gateway(
    data_lake: Arc<DataLake>,
    mut req: Request<Body>,
) -> Response<Body> {
    // Gateway keys are configured as: sk-phantom-key=pool,sk-phantom-other=other-pool.
    let auth_header = match req
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
    {
        Some(value) => value,
        None => return error_response(StatusCode::UNAUTHORIZED, "Missing Authorization header"),
    };
    let provided_key = match auth_header.strip_prefix("Bearer ") {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => return error_response(StatusCode::UNAUTHORIZED, "Invalid Authorization header"),
    };
    let configured_keys = std::env::var("PHANTOM_GATEWAY_KEYS").unwrap_or_default();
    let pool_tag = match resolve_gateway_pool(&configured_keys, &provided_key) {
        Some(pool) => pool,
        None if configured_keys.trim().is_empty() => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Gateway API keys are not configured",
            );
        }
        None => return error_response(StatusCode::UNAUTHORIZED, "Invalid API key"),
    };
    // 2. 网关自动轮询负载均衡：拉取当前可用的活跃账号
    let mut attempts = 0;
    let max_retry_accounts = 3;

    loop {
        attempts += 1;
        let target_account = match data_lake.lease_active_account_for_routing(&pool_tag).await {
            Ok(Some(account)) => account,
            Ok(None) => {
                return Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .body(Body::from(format!(
                        r#"{{"error": {{"message": "No active and non-cooling accounts available in pool '{pool_tag}'"}}}}"#
                    )))
                    .unwrap();
            }
            Err(error) => {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from(format!(
                        r#"{{"error": {{"message": "Database query failed: {error}"}}}}"#
                    )))
                    .unwrap();
            }
        };
        // 2.5 动态并发与平滑限流（Dynamic Rate Limiting）
        if attempts == 1 {
            let max_burst = 5.0;
            let refill_rate = 2.0;

            if !check_rate_limit(&provided_key, max_burst, refill_rate) {
                return Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .body(Body::from(r#"{"error": {"message": "网关并发流量超限，请降低请求频率 (Gateway Rate Limit Exceeded)"}}"#))
                    .unwrap();
            }
        }

        // 选择使用频率最低或最久未使用的账号（list_active_accounts_for_routing 已经按 last_used_at 升序排列，即最先使用的是最久未使用的）
        // 3. 准备转发请求至 OpenAI 官方端点
        let access_token = match &target_account.access_token {
            Some(at) if !at.trim().is_empty() => at.clone(),
            _ => {
                // 如果 access_token 为空但有 st 或 rt，尝试主动自愈刷新
                let _ = data_lake
                    .update_account_gateway_activity(
                        &target_account.id,
                        target_account.consecutive_failures.unwrap_or(0) + 1,
                        Some("Access Token is empty"),
                    )
                    .await;
                if check_account_status(data_lake.clone(), &target_account.id)
                    .await
                    .is_err()
                {
                    continue; // 换下一个账号重试
                }
                // 再次读取
                match data_lake.get_generated_account(&target_account.id).await {
                    Ok(Some(updated_acc)) => {
                        if let Some(at) = updated_acc.access_token {
                            at
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
            }
        };

        // 更新最后使用时间戳与今日计数
        // 构建转发的请求
        let proxy_url = target_account.proxy_url.as_deref();
        let client =
            crate::openai::impersonator::ImpersonateProvider::create_chrome_client(proxy_url);

        // 我们需要重新构建请求体
        let uri = "https://api.openai.com/v1/chat/completions";

        // 构造纯净的代理请求头，剔除所有客户端指纹（如 Origin, Referer, CF-Connecting-IP, Accept-Encoding 等）
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "authorization",
            reqwest::header::HeaderValue::from_str(&format!("Bearer {access_token}")).unwrap(),
        );
        if let Some(ct) = req.headers().get("content-type") {
            headers.insert("content-type", ct.clone());
        } else {
            headers.insert(
                "content-type",
                reqwest::header::HeaderValue::from_static("application/json"),
            );
        }
        if let Some(accept) = req.headers().get("accept") {
            headers.insert("accept", accept.clone());
        }

        // 读取 Body
        let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
            Ok(b) => b,
            Err(e) => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!(
                        r#"{{"error": {{"message": "Failed to read request body: {e}"}}}}"#
                    )))
                    .unwrap();
            }
        };

        // 发送给 OpenAI
        match client
            .post(uri)
            .headers(headers)
            .body(body_bytes.clone())
            .send()
            .await
        {
            Ok(openai_resp) => {
                let status = openai_resp.status();

                // 4. 自愈处理 401 令牌过期或 429 频率限制
                if status == StatusCode::UNAUTHORIZED {
                    // 令牌已失效，触发主动自愈，并增加错误次数，换下一个账号
                    let _ = data_lake
                        .update_account_gateway_activity(
                            &target_account.id,
                            5,
                            Some("OpenAI returned 401 Unauthorized"),
                        )
                        .await;
                    // 异步触发自愈刷新
                    let dl = data_lake.clone();
                    let acc_id = target_account.id.clone();
                    tokio::spawn(async move {
                        let _ = check_account_status(dl, &acc_id).await;
                    });
                    if attempts < max_retry_accounts {
                        // 重新装填 req
                        req = Request::new(Body::from(body_bytes));
                        continue;
                    }
                } else if status == StatusCode::TOO_MANY_REQUESTS {
                    // 触发智能冷却：标记当前账号冷却 60 秒
                    let _ = data_lake
                        .mark_account_cooling_down(&target_account.id, 60)
                        .await;
                    let _ = data_lake
                        .update_account_gateway_activity(
                            &target_account.id,
                            0,
                            Some("OpenAI returned 429 Rate Limit"),
                        )
                        .await;
                    if attempts < max_retry_accounts {
                        // 重新装填 req
                        req = Request::new(Body::from(body_bytes));
                        continue;
                    }
                }
                if target_account.consecutive_failures.unwrap_or(0) > 0 {
                    let _ = data_lake
                        .update_account_gateway_activity(
                            &target_account.id,
                            0,
                            Some("Recovered and Success"),
                        )
                        .await;
                }

                // 正常响应转发
                let mut resp_builder = Response::builder().status(status);
                for (k, v) in openai_resp.headers().iter() {
                    resp_builder = resp_builder.header(k.as_str(), v.as_ref());
                }

                let stream = openai_resp.bytes_stream();
                return resp_builder.body(Body::from_stream(stream)).unwrap();
            }
            Err(e) => {
                // 网络错误或代理失效
                let _ = data_lake
                    .update_account_gateway_activity(
                        &target_account.id,
                        target_account.consecutive_failures.unwrap_or(0) + 1,
                        Some(&format!("Network error: {e}")),
                    )
                    .await;

                if attempts < max_retry_accounts {
                    // 重新装填 req
                    req = Request::new(Body::from(body_bytes));
                    continue;
                }

                return Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::from(format!(
                        r#"{{"error": {{"message": "Failed to connect to OpenAI endpoint: {e}"}}}}"#
                    )))
                    .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{constant_time_eq, resolve_gateway_pool};

    #[test]
    fn resolves_configured_gateway_key_to_pool() {
        let configured = "sk-phantom-main=default,sk-phantom-team=team";
        assert_eq!(
            resolve_gateway_pool(configured, "sk-phantom-team").as_deref(),
            Some("team")
        );
        assert_eq!(resolve_gateway_pool(configured, "sk-phantom-missing"), None);
    }

    #[test]
    fn constant_time_comparison_checks_length_and_content() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secrex"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
    }
}
