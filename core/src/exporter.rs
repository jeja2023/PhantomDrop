use serde::{Serialize, Deserialize};
use crate::db::GeneratedAccountRecord;

/// 支持的导出格式类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExportFormat {
    Cpa,
    Sub2api,
    NewApi,
    KiroGo,
    StandardJson,
}

/// 统一的导出服务，借鉴 any-auto-register 的多平台分发体系设计
pub struct AccountExporter;

impl AccountExporter {
    /// 将数据库记录转换为指定平台的 Payload
    pub fn transform(acc: &GeneratedAccountRecord, format: ExportFormat) -> serde_json::Value {
        match format {
            ExportFormat::Cpa => {
                // CPA 协议通常偏好平铺且包含 account_id (等同于 workspace_id)
                serde_json::json!({
                    "username": acc.address,
                    "password": acc.password,
                    "access_token": acc.access_token.as_deref().unwrap_or(""),
                    "refresh_token": acc.refresh_token.as_deref().unwrap_or(""),
                    "session_token": acc.session_token.as_deref().unwrap_or(""),
                    "account_id": acc.workspace_id.as_deref().unwrap_or(""),
                    "device_id": acc.device_id.as_deref().unwrap_or(""),
                    "platform": "openai",
                    "status": "ready"
                })
            },
            ExportFormat::Sub2api | ExportFormat::NewApi => {
                // Sub2API/NewAPI 偏好 access_token 命名
                serde_json::json!({
                    "email": acc.address,
                    "password": acc.password,
                    "accessToken": acc.access_token.as_deref().unwrap_or(""),
                    "refreshToken": acc.refresh_token.as_deref().unwrap_or(""),
                    "sessionToken": acc.session_token.as_deref().unwrap_or(""),
                    "type": "openai"
                })
            },
            ExportFormat::KiroGo => {
                // Kiro-Go 风格
                serde_json::json!({
                    "account": acc.address,
                    "pass": acc.password,
                    "at": acc.access_token.as_deref().unwrap_or(""),
                    "rt": acc.refresh_token.as_deref().unwrap_or("")
                })
            },
            ExportFormat::StandardJson => {
                serde_json::to_value(acc).unwrap_or(serde_json::json!({}))
            }
        }
    }

    /// 批量转换
    pub fn transform_batch(accounts: &[GeneratedAccountRecord], format: ExportFormat) -> Vec<serde_json::Value> {
        accounts.iter().map(|acc| Self::transform(acc, format)).collect()
    }
}
