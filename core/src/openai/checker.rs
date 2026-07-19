use crate::db::DataLake;
use reqwest::header::{AUTHORIZATION, COOKIE, HeaderMap, HeaderValue};
use std::sync::Arc;

/**
 * OpenAI 账户状态检查器
 * 实现对 Session Token 和 Access Token 的双重效能验证
 * 支持通过 Refresh Token 主动续期 Access Token
 */

pub async fn check_account_status(
    data_lake: Arc<DataLake>,
    account_id: &str,
) -> Result<String, String> {
    // 1. 从数据库读取账号信息
    let mut account = data_lake
        .get_generated_account(account_id)
        .await
        .map_err(|e| format!("数据库读取失败: {e}"))?
        .ok_or_else(|| "账号不存在".to_string())?;

    let mut final_status = "Unknown".to_string();
    let mut details: Vec<String> = Vec::new();

    // 1.5 创建客户端 (关键：使用账号关联的代理)
    let proxy_url = account.proxy_url.as_deref();
    let client = crate::openai::impersonator::ImpersonateProvider::create_chrome_client(proxy_url);

    // 2. 尝试使用 Session Token 校验 (针对 ChatGPT 网页版账号)
    if let Some(ref st) = account.session_token {
        if !st.trim().is_empty() {
            let mut headers = HeaderMap::new();
            headers.insert(
                COOKIE,
                HeaderValue::from_str(&format!("__Secure-next-auth.session-token={st}")).unwrap(),
            );

            match client
                .get("https://chatgpt.com/backend-api/models")
                .headers(headers.clone())
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        final_status = "Active".to_string();
                        details.push("Web 会话有效".to_string());

                        // 自动换取并刷新 Access Token
                        let mut refresh_headers = headers.clone();
                        refresh_headers
                            .insert("accept", HeaderValue::from_static("application/json"));
                        if let Ok(session_resp) = client
                            .get("https://chatgpt.com/api/auth/session")
                            .headers(refresh_headers)
                            .send()
                            .await
                        {
                            if session_resp.status().is_success() {
                                if let Ok(json) = session_resp.json::<serde_json::Value>().await {
                                    let new_at = json.get("accessToken").and_then(|v| v.as_str());
                                    let new_rt = json.get("refreshToken").and_then(|v| v.as_str());

                                    if new_at.is_some() || new_rt.is_some() {
                                        if let Some(at) = new_at {
                                            account.access_token = Some(at.to_string());
                                        }
                                        if let Some(rt) = new_rt {
                                            account.refresh_token = Some(rt.to_string());
                                        }

                                        // 更新数据库中的 token
                                        let _ = data_lake
                                            .update_account_tokens(
                                                account_id,
                                                account.access_token.as_deref(),
                                                account.refresh_token.as_deref(),
                                                Some(st),
                                                account.device_id.as_deref(),
                                                account.workspace_id.as_deref(),
                                                account.id_token.as_deref(),
                                                account.chatgpt_account_id.as_deref(),
                                                account.chatgpt_user_id.as_deref(),
                                                account.organization_id.as_deref(),
                                                account.plan_type.as_deref(),
                                                account.expires_in,
                                                account.token_version,
                                                account.oauth_credentials_json.as_deref(),
                                            )
                                            .await;

                                        details.push("自动更新 Access Token 成功".to_string());
                                    }
                                }
                            }
                        }
                    } else if resp.status() == 401 {
                        final_status = "Expired".to_string();
                        details.push("Web 会话已过期或未授权 (401)".to_string());
                    } else if resp.status() == 403 {
                        final_status = "Banned".to_string();
                        details.push("账号已被封禁或 IP 受限 (403)".to_string());
                    } else {
                        details.push(format!("Web 检查返回异常状态: {}", resp.status()));
                    }
                }
                Err(e) => {
                    details.push("Web 检查网络错误".to_string());
                    eprintln!("🔴 [OpenAI Checker] Session check error: {e}");
                }
            }
        }
    }

    // 2.5 Refresh Token 主动续期 (当 Session 过期但 RT 可用时尝试)
    if final_status == "Expired" || final_status == "Unknown" {
        if let Some(ref rt) = account.refresh_token {
            if rt.starts_with("rt_") && !rt.trim().is_empty() {
                details.push("尝试通过 Refresh Token 续期...".to_string());

                let refresh_result = refresh_access_token_via_rt(&client, rt).await;
                match refresh_result {
                    Ok((new_access, new_refresh, new_id)) => {
                        // 验证新 Access Token 是否有效
                        let mut verify_headers = HeaderMap::new();
                        verify_headers.insert(
                            AUTHORIZATION,
                            HeaderValue::from_str(&format!("Bearer {new_access}")).unwrap(),
                        );

                        let verify_ok = client
                            .get("https://chatgpt.com/backend-api/models")
                            .headers(verify_headers)
                            .send()
                            .await
                            .map(|r| r.status().is_success())
                            .unwrap_or(false);

                        if verify_ok {
                            account.access_token = Some(new_access);
                            if let Some(nrt) = new_refresh {
                                account.refresh_token = Some(nrt);
                            }
                            if let Some(nid) = new_id {
                                account.id_token = Some(nid);
                            }

                            // 从新 ID Token 中提取元数据
                            let auth_info = account
                                .id_token
                                .as_deref()
                                .map(|idt| crate::openai::oauth::extract_auth_info_from_jwt(idt));
                            let chatgpt_account_id = auth_info
                                .as_ref()
                                .and_then(|i| i.chatgpt_account_id.clone())
                                .or_else(|| account.chatgpt_account_id.clone());
                            let chatgpt_user_id = auth_info
                                .as_ref()
                                .and_then(|i| i.chatgpt_user_id.clone())
                                .or_else(|| account.chatgpt_user_id.clone());
                            let organization_id = auth_info
                                .as_ref()
                                .and_then(|i| i.organization_id.clone())
                                .or_else(|| account.organization_id.clone());
                            let plan_type = auth_info
                                .as_ref()
                                .and_then(|i| i.plan_type.clone())
                                .or_else(|| account.plan_type.clone());

                            let _ = data_lake
                                .update_account_tokens(
                                    account_id,
                                    account.access_token.as_deref(),
                                    account.refresh_token.as_deref(),
                                    account.session_token.as_deref(),
                                    account.device_id.as_deref(),
                                    account.workspace_id.as_deref(),
                                    account.id_token.as_deref(),
                                    chatgpt_account_id.as_deref(),
                                    chatgpt_user_id.as_deref(),
                                    organization_id.as_deref(),
                                    plan_type.as_deref(),
                                    account.expires_in,
                                    account.token_version,
                                    account.oauth_credentials_json.as_deref(),
                                )
                                .await;

                            final_status = "Active (RT 续期)".to_string();
                            details.push("Refresh Token 续期成功，已更新 Access Token".to_string());
                        } else {
                            details.push(
                                "Refresh Token 续期后验证失败，Token 可能已被吊销".to_string(),
                            );
                        }
                    }
                    Err(e) => {
                        details.push(format!("Refresh Token 续期失败: {e}"));
                    }
                }
            }
        }
    }

    // 3. 如果 Web 校验没有成功确认状态，尝试使用 Access Token (针对 API)
    if final_status != "Active" && final_status != "Active (RT 续期)" && final_status != "Banned"
    {
        if let Some(ref at) = account.access_token {
            if !at.trim().is_empty() {
                let mut headers = HeaderMap::new();
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {at}")).unwrap(),
                );

                match client
                    .get("https://api.openai.com/v1/models")
                    .headers(headers)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            final_status = "Active (API)".to_string();
                            details.push("API Key 有效".to_string());
                        } else if resp.status() == 401 {
                            final_status = "Expired (API)".to_string();
                            details.push("API Key 已过期或无效 (401)".to_string());
                        } else if resp.status() == 403 {
                            final_status = "Banned (API)".to_string();
                            details.push("账号封禁或权限不足 (403)".to_string());
                        } else {
                            details.push(format!("API 检查返回异常状态: {}", resp.status()));
                        }
                    }
                    Err(e) => {
                        details.push("API 检查网络错误".to_string());
                        eprintln!("🔴 [OpenAI Checker] API check error: {e}");
                    }
                }
            }
        }
    }

    // 4. 更新数据库中的状态
    if final_status != "Unknown" {
        let _ = data_lake
            .update_account_status(account_id, &final_status)
            .await;
    }

    if final_status == "Unknown" {
        if details.is_empty() {
            let _ = data_lake
                .update_account_status(account_id, "No Token")
                .await;
            return Ok("No Token".to_string());
        }
        let detailed_status = format!("Unknown: {}", details.join(", "));
        let _ = data_lake
            .update_account_status(account_id, &detailed_status)
            .await;
        return Ok(detailed_status);
    }

    Ok(final_status)
}

/// 通过 Refresh Token 向 OpenAI OAuth 端点请求新的 Access Token
/// 返回 (access_token, Option<refresh_token>, Option<id_token>)
async fn refresh_access_token_via_rt(
    client: &reqwest::Client,
    refresh_token: &str,
) -> Result<(String, Option<String>, Option<String>), String> {
    let token_url = crate::openai::constants::AUTH_TOKEN_URL;
    let client_id = crate::openai::constants::OPENAI_CLIENT_ID;

    let payload = [
        ("grant_type", "refresh_token"),
        ("client_id", client_id),
        ("refresh_token", refresh_token),
    ];

    let response = client
        .post(token_url)
        .form(&payload)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("origin", "https://chatgpt.com")
        .send()
        .await
        .map_err(|e| format!("RT 续期请求网络错误: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("RT 续期被拒绝: {}", response.status()));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("RT 续期响应解析失败: {e}"))?;

    let access_token = data
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "RT 续期响应中未包含 access_token".to_string())?;

    let new_refresh = data
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let new_id_token = data
        .get("id_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok((access_token, new_refresh, new_id_token))
}
