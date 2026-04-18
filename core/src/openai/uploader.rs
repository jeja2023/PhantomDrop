use reqwest::Client;
use serde_json::json;

/// 账号上传分发管理器
#[allow(dead_code)]
pub struct AccountUploader {
    client: Client,
}

#[allow(dead_code)]
impl AccountUploader {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }

    /// 执行 CPA 格式的账号上传 (multipart/form-data)
    pub async fn upload_cpa(
        &self,
        api_url: &str,
        api_key: &str,
        email: &str,
        password: &str,
        access_token: Option<&str>,
        session_token: Option<&str>,
    ) -> Result<(), String> {
        let auth_header = format!("Bearer {}", api_key);
        let form = reqwest::multipart::Form::new()
            .text("username", email.to_string())
            .text("password", password.to_string())
            .text("access_token", access_token.unwrap_or("").to_string())
            .text("session_token", session_token.unwrap_or("").to_string())
            .text("status", "ready");

        let response = self
            .client
            .post(api_url)
            .header("Authorization", auth_header)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("CPA 上传网络失败: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("CPA 服务器拒绝: {}", response.status()));
        }

        Ok(())
    }

    /// 执行 NewAPI / Sub2API 格式的账号上传 (application/json)
    pub async fn upload_newapi(
        &self,
        api_url: &str,
        api_key: &str,
        email: &str,
        password: &str,
        access_token: Option<&str>,
        session_token: Option<&str>,
    ) -> Result<(), String> {
        let payload = json!({
            "accounts": [{
                "email": email,
                "password": password,
                "access_token": access_token,
                "session_token": session_token
            }]
        });

        let response = self
            .client
            .post(api_url)
            .header("x-api-key", api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("NewAPI 上传网络失败: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("NewAPI 服务器拒绝: {}", response.status()));
        }

        Ok(())
    }
}
