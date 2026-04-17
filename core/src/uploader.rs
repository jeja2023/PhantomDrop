use reqwest::Client;
use serde_json::json;

/// 将生成的账号分发/推送到指定的 CPA (分发平台)
pub async fn upload_account(
    client: &Client,
    cpa_url: &str,
    cpa_key: &str,
    email: &str,
    password: &str,
    access_token: Option<&str>,
    refresh_token: Option<&str>,
    session_token: Option<&str>,
) -> Result<(), String> {
    // 构建兼容性更强的平铺 Payload
    let payload = json!({
        "username": email,
        "email": email, // 兼容某些平台要求 email 字段
        "password": password,
        "access_token": access_token.unwrap_or(""),
        "accessToken": access_token.unwrap_or(""), // 兼容驼峰命名
        "refresh_token": refresh_token.unwrap_or(""),
        "refreshToken": refresh_token.unwrap_or(""), // 兼容驼峰命名
        "session_token": session_token.unwrap_or(""),
        "sessionToken": session_token.unwrap_or(""), // 兼容驼峰命名
        "status": "ready",
        "type": "openai"
    });

    let res = client
        .post(cpa_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", cpa_key))
        .header("x-api-key", cpa_key) // 同时发送两种常见的 API KEY Header 提高兼容性
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("分发请求失败 (网络原因): {}", e))?;

    if res.status().is_success() {
        Ok(())
    } else {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        Err(format!("CPA 平台拒绝 (HTTP {}): {}", status, body))
    }
}
