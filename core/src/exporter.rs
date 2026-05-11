use crate::db::GeneratedAccountRecord;
use serde::{Deserialize, Serialize};

/// 账号导出格式枚举。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExportFormat {
    Cpa,
    Sub2api,
    NewApi,
    KiroGo,
    StandardJson,
    OauthJson,
}

pub struct AccountExporter;

impl AccountExporter {
    /// 将数据库账号记录转换为指定平台需要的负载结构。
    pub fn transform(acc: &GeneratedAccountRecord, format: ExportFormat) -> serde_json::Value {
        match format {
            ExportFormat::Cpa => {
                let account_id = acc.workspace_id.as_deref().unwrap_or("");
                let mock_id_token =
                    "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.compat_signature_for_cpa_parsing_only"
                        .to_string();
                let id_token = acc
                    .id_token
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(mock_id_token.as_str());

                serde_json::json!({
                    "type": "codex",
                    "email": acc.address,
                    "password": acc.password,
                    "access_token": acc.access_token.as_deref().unwrap_or(""),
                    "refresh_token": acc.refresh_token.as_deref().unwrap_or(""),
                    "account_id": account_id,
                    "id_token": id_token,
                    "device_id": acc.device_id.as_deref().unwrap_or(""),
                    "platform": "openai",
                    "status": "ready"
                })
            }
            ExportFormat::Sub2api | ExportFormat::NewApi => {
                serde_json::json!({
                    "email": acc.address,
                    "password": acc.password,
                    "accessToken": acc.access_token.as_deref().unwrap_or(""),
                    "refreshToken": acc.refresh_token.as_deref().unwrap_or(""),
                    "sessionToken": acc.session_token.as_deref().unwrap_or(""),
                    "type": "openai"
                })
            }
            ExportFormat::KiroGo => {
                serde_json::json!({
                    "account": acc.address,
                    "pass": acc.password,
                    "at": acc.access_token.as_deref().unwrap_or(""),
                    "rt": acc.refresh_token.as_deref().unwrap_or("")
                })
            }
            ExportFormat::StandardJson => {
                serde_json::to_value(acc).unwrap_or(serde_json::json!({}))
            }
            ExportFormat::OauthJson => {
                let stored_credentials = acc
                    .oauth_credentials_json
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
                let credentials = crate::openai::oauth::build_oauth_credentials_value(
                    crate::openai::oauth::OAuthCredentialInput {
                        email: &acc.address,
                        access_token: acc.access_token.as_deref(),
                        refresh_token: acc.refresh_token.as_deref(),
                        id_token: acc.id_token.as_deref(),
                        workspace_id: acc.workspace_id.as_deref(),
                        chatgpt_account_id: acc.chatgpt_account_id.as_deref(),
                        chatgpt_user_id: acc.chatgpt_user_id.as_deref(),
                        organization_id: acc.organization_id.as_deref(),
                        plan_type: acc.plan_type.as_deref(),
                        expires_in: acc.expires_in,
                        token_version: acc.token_version,
                        stored_credentials: stored_credentials.as_ref(),
                    },
                );

                serde_json::json!({
                    "name": acc.address,
                    "platform": "openai",
                    "type": "oauth",
                    "credentials": credentials,
                    "extra": {
                        "email": acc.address,
                        "privacy_mode": "training_off"
                    },
                    "concurrency": 10,
                    "priority": 1,
                    "rate_multiplier": 1,
                    "auto_pause_on_expired": true
                })
            }
        }
    }

    #[allow(dead_code)]
    pub fn transform_batch(
        accounts: &[GeneratedAccountRecord],
        format: ExportFormat,
    ) -> Vec<serde_json::Value> {
        accounts
            .iter()
            .map(|acc| Self::transform(acc, format))
            .collect()
    }

    /// 将账号批量导出为带元数据的 OAuth JSON 包。
    pub fn export_to_oauth_json(accounts: &[GeneratedAccountRecord]) -> serde_json::Value {
        let accounts_json: Vec<serde_json::Value> = accounts
            .iter()
            .map(|acc| Self::transform(acc, ExportFormat::OauthJson))
            .collect();

        serde_json::json!({
            "exported_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "proxies": serde_json::Value::Array(vec![]),
            "accounts": accounts_json
        })
    }
}
