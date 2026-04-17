use crate::db::DataLake;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE};
use std::sync::Arc;

/**
 * OpenAI 账户状态检查器
 * 实现对 Session Token 和 Access Token 的双重效能验证
 */

pub async fn check_account_status(
    data_lake: Arc<DataLake>,
    account_id: &str,
) -> Result<String, String> {
    // 1. 从数据库读取账号信息
    let account = data_lake.get_generated_account(account_id).await
        .map_err(|e| format!("数据库读取失败: {}", e))?
        .ok_or_else(|| "账号不存在".to_string())?;

    let client = crate::openai::impersonator::ImpersonateProvider::create_chrome_client(None);

    let mut final_status = "Unknown".to_string();
    let mut details: Vec<String> = Vec::new();

    // 2. 尝试使用 Session Token 校验 (针对 ChatGPT 网页版账号)
    if let Some(ref st) = account.session_token {
        if !st.trim().is_empty() {
            let mut headers = HeaderMap::new();
            headers.insert(
                COOKIE,
                HeaderValue::from_str(&format!("__Secure-next-auth.session-token={}", st)).unwrap(),
            );

            match client
                .get("https://chatgpt.com/backend-api/models")
                .headers(headers)
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        final_status = "Active".to_string();
                        details.push("Web 会话有效".to_string());
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
                    eprintln!("🔴 [OpenAI Checker] Session check error: {}", e);
                }
            }
        }
    }

    // 3. 如果 Web 校验没有成功确认状态，尝试使用 Access Token (针对 API)
    if final_status != "Active" && final_status != "Banned" {
        if let Some(ref at) = account.access_token {
            if !at.trim().is_empty() {
                let mut headers = HeaderMap::new();
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", at)).unwrap(),
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
                        eprintln!("🔴 [OpenAI Checker] API check error: {}", e);
                    }
                }
            }
        }
    }

    // 4. 更新数据库中的状态
    if final_status != "Unknown" {
        let display_status = if details.is_empty() {
            final_status.clone()
        } else {
             // 如果是 Banned 则标记为异常
             final_status.clone()
        };
        
        let _ = data_lake
            .update_account_status(account_id, &display_status)
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
