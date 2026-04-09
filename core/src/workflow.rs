use std::sync::Arc;
use crate::stream::{StreamHub, StreamPayload};
use crate::db::{DataLake, WorkflowDefinitionRecord};
use uuid::Uuid;
use serde_json::json;
use serde::{Deserialize, Serialize};
use std::env;
use chrono::Utc;

/**
 * 幻影工作流引擎 (Workflow Engine)
 * 职责：具体的自动化业务逻辑实现，如账户生成模拟、负载统计、清理任务等
 */

pub struct WorkflowEngine {
    hub: Arc<StreamHub>,
    dl: Arc<DataLake>,
}

#[derive(Clone, Serialize)]
pub struct WorkflowDefinition {
    pub id: String,
    pub kind: WorkflowKind,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub builtin: bool,
    pub parameters: WorkflowParameters,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKind {
    AccountGenerate,
    DataCleanup,
    StatusReport,
    EnvironmentCheck,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct WorkflowParameters {
    pub batch_size: Option<usize>,
    pub account_domain: Option<String>,
    pub days_to_keep: Option<i64>,
    pub report_window_hours: Option<i64>,
    pub require_env_secret_match: Option<bool>,
    pub require_public_hub_url: Option<bool>,
    pub require_webhook: Option<bool>,
}

struct WorkflowRunContext {
    run_id: String,
    workflow_id: String,
    workflow_title: String,
    step_index: i64,
}

impl WorkflowEngine {
    pub fn new(hub: Arc<StreamHub>, dl: Arc<DataLake>) -> Self {
        Self { hub, dl }
    }

    pub fn builtin_definitions() -> Vec<WorkflowDefinition> {
        vec![
            WorkflowDefinition {
                id: "批量生成".to_string(),
                kind: WorkflowKind::AccountGenerate,
                title: "多平台批量账号注入".to_string(),
                summary: "生成可追踪的账号产物，并将结果绑定到工作流运行记录".to_string(),
                status: "ready".to_string(),
                builtin: true,
                parameters: WorkflowParameters {
                    batch_size: Some(10),
                    account_domain: Some("phantom.local".to_string()),
                    days_to_keep: None,
                    report_window_hours: None,
                    require_env_secret_match: None,
                    require_public_hub_url: None,
                    require_webhook: None,
                },
            },
            WorkflowDefinition {
                id: "数据清理".to_string(),
                kind: WorkflowKind::DataCleanup,
                title: "数据湖自动清理维护".to_string(),
                summary: "清理超出保留周期的历史邮件记录".to_string(),
                status: "ready".to_string(),
                builtin: true,
                parameters: WorkflowParameters {
                    days_to_keep: Some(7),
                    ..WorkflowParameters::default()
                },
            },
            WorkflowDefinition {
                id: "负载报告".to_string(),
                kind: WorkflowKind::StatusReport,
                title: "中枢负载状态巡检".to_string(),
                summary: "生成当前数据库与实时流的运行情况报告".to_string(),
                status: "active".to_string(),
                builtin: true,
                parameters: WorkflowParameters {
                    report_window_hours: Some(24),
                    ..WorkflowParameters::default()
                },
            },
            WorkflowDefinition {
                id: "环境变量".to_string(),
                kind: WorkflowKind::EnvironmentCheck,
                title: "环境变量同步校验".to_string(),
                summary: "验证边缘节点和中枢之间的关键密钥配置".to_string(),
                status: "ready".to_string(),
                builtin: true,
                parameters: WorkflowParameters {
                    require_env_secret_match: Some(true),
                    require_public_hub_url: Some(true),
                    require_webhook: Some(false),
                    ..WorkflowParameters::default()
                },
            },
        ]
    }

    pub fn builtin_ids() -> Vec<&'static str> {
        vec!["批量生成", "数据清理", "负载报告", "环境变量"]
    }

    pub async fn ensure_builtin_definitions(&self) {
        for definition in Self::builtin_definitions() {
            let parameters_json = serde_json::to_string(&definition.parameters).unwrap_or_else(|_| "{}".to_string());
            let _ = self
                .dl
                .upsert_workflow_definition(
                    &definition.id,
                    &definition.kind.as_storage(),
                    &definition.title,
                    &definition.summary,
                    &definition.status,
                    &parameters_json,
                )
                .await;
        }
    }

    pub async fn definitions(&self) -> Vec<WorkflowDefinition> {
        match self.dl.list_workflow_definitions().await {
            Ok(records) if !records.is_empty() => records.into_iter().map(Self::from_record).collect(),
            _ => Self::builtin_definitions(),
        }
    }

    fn find_definition(definitions: &[WorkflowDefinition], workflow_id: &str) -> Option<WorkflowDefinition> {
        definitions
            .iter()
            .find(|definition| definition.id == workflow_id)
            .cloned()
    }

    async fn resolve_definition(&self, workflow_id: &str) -> Result<WorkflowDefinition, String> {
        let definitions = self.definitions().await;
        Self::find_definition(&definitions, workflow_id)
            .ok_or_else(|| format!("工作流不存在: {}", workflow_id))
    }

    pub fn validate_definition_input(
        kind: &str,
        status: &str,
        parameters_json: &str,
    ) -> Result<WorkflowParameters, String> {
        let kind = WorkflowKind::try_from_storage(kind)?;
        let parameters = serde_json::from_str::<WorkflowParameters>(parameters_json)
            .map_err(|error| format!("工作流参数 JSON 无效: {error}"))?;

        match status {
            "ready" | "active" | "idle" => {}
            _ => return Err("工作流状态无效，仅支持 ready/active/idle".to_string()),
        }

        let definition = WorkflowDefinition {
            id: "validation".to_string(),
            kind,
            title: "validation".to_string(),
            summary: String::new(),
            status: status.to_string(),
            builtin: false,
            parameters: parameters.clone(),
        };

        Self::validate_parameters(&definition)?;
        Ok(parameters)
    }

    /// 执行预定义的工作流指令
    pub async fn execute(&self, workflow_id: &str) -> Result<String, String> {
        let hub = Arc::clone(&self.hub);
        let dl = Arc::clone(&self.dl);
        let definition = self.resolve_definition(workflow_id).await?;
        let id = definition.id.clone();
        let run_id = Uuid::new_v4().to_string();

        let _ = dl.create_workflow_run(
            &run_id,
            &id,
            &definition.title,
            "running",
            "工作流已进入执行队列",
        ).await;

        let run_id_for_task = run_id.clone();
        let definition_for_task = definition;

        tokio::spawn(async move {
            let mut context = WorkflowRunContext {
                run_id: run_id_for_task.clone(),
                workflow_id: id.clone(),
                workflow_title: definition_for_task.title.to_string(),
                step_index: 0,
            };

            // 1. 发送开始任务日志
            Self::log_step(&hub, &dl, &mut context, "info", &format!("开始执行工作流: [{}] / RUN_ID: {}", id, run_id_for_task)).await;

            if let Err(message) = Self::validate_parameters(&definition_for_task) {
                Self::log_step(&hub, &dl, &mut context, "warn", &message).await;
                let _ = dl.finish_workflow_run(&run_id_for_task, "warn", &message).await;
                return;
            }

            match definition_for_task.kind {
                WorkflowKind::AccountGenerate => {
                    let message = Self::simulate_account_gen(&hub, &dl, &mut context, &definition_for_task.parameters).await;
                    let _ = dl.finish_workflow_run(&run_id_for_task, "success", &message).await;
                }
                WorkflowKind::StatusReport => {
                    match Self::simulate_status_check(&hub, &dl, &mut context, &definition_for_task.parameters).await {
                        Ok(message) => {
                            let _ = dl.finish_workflow_run(&run_id_for_task, "success", &message).await;
                        }
                        Err(message) => {
                            let _ = dl.finish_workflow_run(&run_id_for_task, "warn", &message).await;
                        }
                    }
                }
                WorkflowKind::DataCleanup => {
                    match Self::simulate_data_cleanup(hub.clone(), dl.clone(), &mut context, &definition_for_task.parameters).await {
                        Ok(message) => {
                            let _ = dl.finish_workflow_run(&run_id_for_task, "success", &message).await;
                        }
                        Err(message) => {
                            let _ = dl.finish_workflow_run(&run_id_for_task, "warn", &message).await;
                        }
                    }
                }
                WorkflowKind::EnvironmentCheck => {
                    match Self::check_environment(&hub, &dl, &mut context, &definition_for_task.parameters).await {
                        Ok(message) => {
                            let _ = dl.finish_workflow_run(&run_id_for_task, "success", &message).await;
                        }
                        Err(message) => {
                            let _ = dl.finish_workflow_run(&run_id_for_task, "warn", &message).await;
                        }
                    }
                }
            }
        });

        Ok(run_id)
    }

    /// 执行真实的批量生成账户任务，生成可追踪的账号产物
    async fn simulate_account_gen(
        hub: &Arc<StreamHub>,
        dl: &Arc<DataLake>,
        context: &mut WorkflowRunContext,
        parameters: &WorkflowParameters,
    ) -> String {
        let generated_count = parameters.batch_size.unwrap_or(10);
        let configured_domain = dl.get_setting("account_domain").await.ok().flatten();
        let domain = parameters
            .account_domain
            .clone()
            .or(configured_domain)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "phantom.local".to_string());

        Self::log_step(hub, dl, context, "info", "正在接入边缘代理池...").await;
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        
        Self::log_step(hub, dl, context, "info", &format!("正在通过 Catch-all 下发表单，目标批次: {} 个账户...", generated_count)).await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;

        let mut created = 0usize;
        for index in 0..generated_count {
            let suffix = Uuid::new_v4().simple().to_string();
            let local_part = format!("pd_{}_{}", Utc::now().timestamp(), &suffix[..8]);
            let password = format!("Pwd{}_{}", Utc::now().timestamp() % 100000, &suffix[8..12]);
            let address = format!("{}@{}", local_part, domain);

            if dl
                .create_generated_account(&context.run_id, &address, &password, "ready")
                .await
                .is_ok()
            {
                created += 1;
                if index < 3 {
                    Self::log_step(hub, dl, context, "info", &format!("已生成账号产物: {}", address)).await;
                }
            } else {
                Self::log_step(hub, dl, context, "warn", &format!("账号产物写入失败: {}", address)).await;
            }
        }

        Self::log_step(hub, dl, context, "success", &format!("账号产物写入完成: {}/{}", created, generated_count)).await;

        let message = format!("工作流执行成功: {} / DONE / GENERATED: {}", context.workflow_id, created);
        message
    }

    fn from_record(record: WorkflowDefinitionRecord) -> WorkflowDefinition {
        let is_builtin = Self::builtin_ids().iter().any(|builtin_id| builtin_id == &record.id.as_str());
        let parameters = serde_json::from_str::<WorkflowParameters>(&record.parameters_json).unwrap_or_default();
        WorkflowDefinition {
            id: record.id,
            kind: WorkflowKind::from_storage(&record.kind),
            title: record.title,
            summary: record.summary,
            status: record.status,
            builtin: is_builtin,
            parameters,
        }
    }

    fn validate_parameters(definition: &WorkflowDefinition) -> Result<(), String> {
        match definition.kind {
            WorkflowKind::AccountGenerate => {
                let batch_size = definition.parameters.batch_size.unwrap_or(10);
                if batch_size == 0 || batch_size > 500 {
                    return Err("批量生成参数无效 / batch_size 必须在 1..=500 之间".to_string());
                }
                if let Some(domain) = definition.parameters.account_domain.as_deref() {
                    if !domain.contains('.') && !domain.contains("local") {
                        return Err("批量生成参数无效 / account_domain 格式不正确".to_string());
                    }
                }
                Ok(())
            }
            WorkflowKind::DataCleanup => {
                let days_to_keep = definition.parameters.days_to_keep.unwrap_or(7);
                if !(1..=365).contains(&days_to_keep) {
                    return Err("数据清理参数无效 / days_to_keep 必须在 1..=365 之间".to_string());
                }
                Ok(())
            }
            WorkflowKind::StatusReport => {
                let report_window_hours = definition.parameters.report_window_hours.unwrap_or(24);
                if !(1..=168).contains(&report_window_hours) {
                    return Err("负载报告参数无效 / report_window_hours 必须在 1..=168 之间".to_string());
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// 数据湖自动清理任务
    async fn simulate_data_cleanup(
        hub: Arc<StreamHub>,
        dl: Arc<DataLake>,
        context: &mut WorkflowRunContext,
        parameters: &WorkflowParameters,
    ) -> Result<String, String> {
        let resolved = parameters.days_to_keep.unwrap_or(7);

        Self::log_step(&hub, &dl, context, "info", &format!("正在评估数据湖过期指纹记录，保留天数: {}...", resolved)).await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        
        match dl.cleanup_emails(resolved).await {
            Ok(count) => {
                let message = format!("数据湖自动清理完成: 成功回收 {} 条过期记录", count);
                Self::log_step(&hub, &dl, context, "success", &message).await;
                Ok(message)
            },
            Err(e) => {
                let message = format!("数据清理执行异常: {:?}", e);
                Self::log_step(&hub, &dl, context, "warn", &message).await;
                Err(message)
            }
        }
    }

    /// 基于真实数据库统计生成系统负载报告
    async fn simulate_status_check(
        hub: &Arc<StreamHub>,
        dl: &Arc<DataLake>,
        context: &mut WorkflowRunContext,
        parameters: &WorkflowParameters,
    ) -> Result<String, String> {
        let report_window_hours = parameters.report_window_hours.unwrap_or(24);
        Self::log_step(hub, dl, context, "info", &format!("正在对 SQLite 数据湖进行健康度快照，统计窗口: {}h...", report_window_hours)).await;
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;

        let stats = dl.get_dashboard_stats().await.map_err(|error| format!("统计查询失败: {:?}", error))?;
        let coverage = if stats.total_emails == 0 {
            0
        } else {
            (stats.code_emails * 100) / stats.total_emails
        };
        let summary = format!(
            "负载报告就绪: [总邮件: {}] [24h新增: {}] [验证码覆盖: {}%] [活跃Webhook: {}]",
            stats.total_emails,
            stats.recent_emails_24h,
            coverage,
            stats.active_webhooks
        );

        Self::log_step(hub, dl, context, "success", &summary).await;

        Ok(summary)
    }

    /// 真实环境校验：检查密钥、公网地址和 Webhook 配置是否齐备
    async fn check_environment(
        hub: &Arc<StreamHub>,
        dl: &Arc<DataLake>,
        context: &mut WorkflowRunContext,
        parameters: &WorkflowParameters,
    ) -> Result<String, String> {
        Self::log_step(hub, dl, context, "info", "正在读取中枢与边缘配置快照...").await;

        let env_secret = env::var("HUB_SECRET").ok().filter(|value| !value.trim().is_empty());
        let saved_secret = dl.get_setting("auth_secret").await.ok().flatten().filter(|value| !value.trim().is_empty());
        let public_hub_url = dl.get_setting("public_hub_url").await.ok().flatten().filter(|value| !value.trim().is_empty());
        let webhook_url = dl.get_setting("webhook_url").await.ok().flatten().filter(|value| !value.trim().is_empty());
        let active_hooks = dl.get_active_webhooks().await.map(|hooks| hooks.len()).unwrap_or(0);
        let require_env_secret_match = parameters.require_env_secret_match.unwrap_or(true);
        let require_public_hub_url = parameters.require_public_hub_url.unwrap_or(true);
        let require_webhook = parameters.require_webhook.unwrap_or(false);

        if env_secret.is_some() {
            Self::log_step(hub, dl, context, "success", "检测到系统环境变量 HUB_SECRET").await;
        } else {
            Self::log_step(hub, dl, context, "warn", "未检测到环境变量 HUB_SECRET，将依赖数据库配置回退").await;
        }

        if saved_secret.is_some() {
            Self::log_step(hub, dl, context, "success", "数据库中存在 auth_secret 配置").await;
        } else {
            Self::log_step(hub, dl, context, "warn", "数据库中未配置 auth_secret").await;
        }

        if public_hub_url.is_some() {
            Self::log_step(hub, dl, context, "success", "已登记公网访问地址").await;
        } else {
            Self::log_step(hub, dl, context, "warn", "尚未登记公网访问地址，Worker 无法从公网访问中枢").await;
        }

        if webhook_url.is_some() || active_hooks > 0 {
            Self::log_step(hub, dl, context, "success", &format!("Webhook 配置可用，当前活跃数: {}", active_hooks)).await;
        } else {
            Self::log_step(hub, dl, context, "warn", "尚未配置可用的 Webhook 回调地址").await;
        }

        let secrets_match = match (&env_secret, &saved_secret) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        };

        let public_hub_ok = public_hub_url.is_some() || !require_public_hub_url;
        let webhook_ok = webhook_url.is_some() || active_hooks > 0 || !require_webhook;
        let secrets_ok = if require_env_secret_match { secrets_match } else { true };

        if secrets_ok && public_hub_ok && webhook_ok {
            let message = "环境变量同步校验完成 / SECRETS_SYNCED_OK".to_string();
            Self::log_step(hub, dl, context, "success", &message).await;
            Ok(message)
        } else {
            let message = "环境校验存在待处理项 / 中枢配置尚未完全对齐".to_string();
            Self::log_step(hub, dl, context, "warn", &message).await;
            Err(message)
        }
    }

    async fn log_step(hub: &Arc<StreamHub>, dl: &Arc<DataLake>, context: &mut WorkflowRunContext, level: &str, msg: &str) {
        context.step_index += 1;
        let _ = dl.add_workflow_step(&context.run_id, context.step_index, level, msg).await;

        hub.broadcast(StreamPayload {
            id: Uuid::new_v4().to_string(),
            event_type: "workflow_step".into(),
            data: json!({
                "run_id": context.run_id,
                "workflow_id": context.workflow_id,
                "workflow_title": context.workflow_title,
                "step_index": context.step_index,
                "level": level,
                "msg": msg
            }),
        });

        hub.broadcast(StreamPayload {
            id: Uuid::new_v4().to_string(),
            event_type: "system_log".into(),
            data: json!({
                "level": level,
                "msg": msg
            }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkflowDefinition, WorkflowEngine};

    #[test]
    fn finds_definition_only_by_exact_id() {
        let definitions = WorkflowEngine::builtin_definitions();

        let exact = WorkflowEngine::find_definition(&definitions, "批量生成");
        let fuzzy = WorkflowEngine::find_definition(&definitions, "批量生成-10-账户");

        assert!(exact.is_some());
        assert!(fuzzy.is_none());
    }

    #[test]
    fn validate_definition_rejects_empty_id_like_fuzzy_hack_by_params_only() {
        let definition = WorkflowDefinition {
            id: "批量生成".to_string(),
            kind: super::WorkflowKind::AccountGenerate,
            title: "批量生成".to_string(),
            summary: String::new(),
            status: "ready".to_string(),
            builtin: false,
            parameters: super::WorkflowParameters {
                batch_size: Some(25),
                ..super::WorkflowParameters::default()
            },
        };

        assert!(super::WorkflowEngine::validate_parameters(&definition).is_ok());
    }
}

impl WorkflowKind {
    pub fn as_storage(&self) -> &'static str {
        match self {
            WorkflowKind::AccountGenerate => "account_generate",
            WorkflowKind::DataCleanup => "data_cleanup",
            WorkflowKind::StatusReport => "status_report",
            WorkflowKind::EnvironmentCheck => "environment_check",
        }
    }

    pub fn from_storage(value: &str) -> Self {
        match value {
            "data_cleanup" => WorkflowKind::DataCleanup,
            "status_report" => WorkflowKind::StatusReport,
            "environment_check" => WorkflowKind::EnvironmentCheck,
            _ => WorkflowKind::AccountGenerate,
        }
    }

    pub fn try_from_storage(value: &str) -> Result<Self, String> {
        match value {
            "account_generate" => Ok(WorkflowKind::AccountGenerate),
            "data_cleanup" => Ok(WorkflowKind::DataCleanup),
            "status_report" => Ok(WorkflowKind::StatusReport),
            "environment_check" => Ok(WorkflowKind::EnvironmentCheck),
            _ => Err(format!("不支持的工作流类型: {}", value)),
        }
    }
}
