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
    let payload = json!({
        "account": {
            "email": email,
            "password": password,
            "access_token": access_token,
            "refresh_token": refresh_token,
            "session_token": session_token,
        }
    });

    let res = client
        .post(cpa_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", cpa_key))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("分发请求失败: {}", e))?;

    if res.status().is_success() {
        Ok(())
    } else {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        Err(format!("分发失败: HTTP {} - {}", status, body))
    }
}
