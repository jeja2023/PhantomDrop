use crate::db::{DataLake, WorkflowDefinitionRecord};
use crate::stream::{StreamHub, StreamPayload};
use chrono::Utc;
use futures_util::StreamExt;
use rand::{Rng, distributions::Alphanumeric};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

/**
 * 幻影工作流引擎 (Workflow Engine)
 * 职责：具体的自动化业务逻辑实现，如账户生成模拟、负载统计、清理任务等
 */

pub struct WorkflowEngine {
    hub: Arc<StreamHub>,
    dl: Arc<DataLake>,
    run_slots: Arc<Semaphore>,
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
    #[serde(rename = "openai_register")]
    OpenAIRegister,
    #[serde(rename = "openai_register_browser")]
    OpenAIRegisterBrowser,
    #[serde(rename = "grok_register")]
    GrokRegister,
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
    /// OpenAI 注册专用：代理服务器地址
    pub proxy_url: Option<String>,
    /// OpenAI 注册专用：打码平台 API Key
    pub captcha_key: Option<String>,
    /// OpenAI 注册专用：接码平台 API Key (SMS-Activate)
    pub sms_key: Option<String>,
    /// 账号分发专用：接收平台 URL
    pub cpa_url: Option<String>,
    /// 账号分发专用：接收平台 API Key
    pub cpa_key: Option<String>,
    /// 对并发执行任务的支持
    pub concurrency: Option<usize>,
    /// 用户个人资料：全名
    pub full_name: Option<String>,
    /// 用户个人资料：年龄
    pub age: Option<i32>,
    /// 注册类型：Free, Plus, API 等
    pub account_type: Option<String>,
    /// 浏览器专用：是否开启无头模式
    pub headless: Option<bool>,
    /// Grok 注册专用：本地 Turnstile Solver 地址
    pub turnstile_solver_url: Option<String>,
    /// Grok 注册专用：单账号执行超时秒数
    pub registration_timeout_secs: Option<u64>,
    /// 混合模式专用：OAuth 重构授权链接
    pub oauth_authorize_url: Option<String>,
    /// 混合模式专用：OAuth code_verifier
    pub oauth_code_verifier: Option<String>,
    /// 混合模式专用：OAuth 平台分类
    pub oauth_platform: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct GrokReadinessCheck {
    pub id: &'static str,
    pub status: &'static str,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct GrokReadinessReport {
    pub ready: bool,
    pub domain: Option<String>,
    pub domain_source: Option<&'static str>,
    pub solver_mode: &'static str,
    pub checks: Vec<GrokReadinessCheck>,
}

struct ResolvedAccountDomain {
    value: String,
    source: &'static str,
}

fn trimmed_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalized_domain(value: Option<&str>) -> Option<String> {
    trimmed_value(value).map(|value| value.trim_end_matches('.').to_ascii_lowercase())
}

fn select_registration_domain(
    workflow_domain: Option<&str>,
    account_domain: Option<&str>,
    cloudflare_zone_domain: Option<&str>,
) -> Option<ResolvedAccountDomain> {
    [
        (workflow_domain, "workflow"),
        (account_domain, "account_domain"),
        (cloudflare_zone_domain, "cloudflare_zone_domain"),
    ]
    .into_iter()
    .find_map(|(candidate, source)| {
        normalized_domain(candidate).map(|value| ResolvedAccountDomain { value, source })
    })
}

fn validate_public_mail_domain(domain: &str) -> Result<(), String> {
    if domain.len() > 253
        || domain.parse::<std::net::IpAddr>().is_ok()
        || domain.contains(['/', '\\', '@', ':'])
        || domain.chars().any(char::is_whitespace)
    {
        return Err("account_domain 必须是域名，不能包含协议、端口、路径或邮箱前缀".to_string());
    }

    let labels = domain.split('.').collect::<Vec<_>>();
    if labels.len() < 2 || domain == "localhost" || domain.ends_with(".local") {
        return Err(
            "account_domain 必须是公网可收信域名，不能使用 localhost 或 .local".to_string(),
        );
    }
    if labels.iter().any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err("account_domain 格式不正确".to_string());
    }
    Ok(())
}

async fn resolve_registration_domain(
    dl: &Arc<DataLake>,
    parameters: &WorkflowParameters,
) -> Result<ResolvedAccountDomain, String> {
    let account_domain = dl.get_setting("account_domain").await.ok().flatten();
    let cloudflare_zone_domain = dl
        .get_setting("cloudflare_zone_domain")
        .await
        .ok()
        .flatten();
    let resolved = select_registration_domain(
        parameters.account_domain.as_deref(),
        account_domain.as_deref(),
        cloudflare_zone_domain.as_deref(),
    )
    .ok_or_else(|| "注册前必须配置可收信的 account_domain 或 cloudflare_zone_domain".to_string())?;
    validate_public_mail_domain(&resolved.value)?;
    Ok(resolved)
}

struct WorkflowRunContext {
    run_id: String,
    workflow_id: String,
    workflow_title: String,
    step_index: i64,
}

impl WorkflowEngine {
    pub fn new(hub: Arc<StreamHub>, dl: Arc<DataLake>) -> Self {
        let max_runs = env::var("MAX_CONCURRENT_WORKFLOWS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(2);
        Self {
            hub,
            dl,
            run_slots: Arc::new(Semaphore::new(max_runs)),
        }
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
                    ..WorkflowParameters::default()
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
            WorkflowDefinition {
                id: "openai_register_default".to_string(),
                kind: WorkflowKind::OpenAIRegister,
                title: "OpenAI 自动化注册".to_string(),
                summary: "全自动解算 PoW 并完成 OpenAI 账号注册流程".to_string(),
                status: "ready".to_string(),
                builtin: true,
                parameters: WorkflowParameters::default(),
            },
            WorkflowDefinition {
                id: "openai_browser_register".to_string(),
                kind: WorkflowKind::OpenAIRegisterBrowser,
                title: "OpenAI 浏览器模拟注册".to_string(),
                summary: "使用有头/无头浏览器仿真操作，绕过高级协议检测".to_string(),
                status: "ready".to_string(),
                builtin: true,
                parameters: WorkflowParameters::default(),
            },
            WorkflowDefinition {
                id: "grok_register_default".to_string(),
                kind: WorkflowKind::GrokRegister,
                title: "Grok 自动化注册".to_string(),
                summary: "自动完成 xAI 邮箱验证、Turnstile 与 Grok SSO 提取".to_string(),
                status: "ready".to_string(),
                builtin: true,
                parameters: WorkflowParameters {
                    account_type: Some("grok_free".to_string()),
                    registration_timeout_secs: Some(180),
                    ..WorkflowParameters::default()
                },
            },
        ]
    }

    pub fn builtin_ids() -> Vec<&'static str> {
        vec![
            "批量生成",
            "数据清理",
            "负载报告",
            "环境变量",
            "openai_register_default",
            "openai_browser_register",
            "grok_register_default",
        ]
    }

    pub async fn ensure_builtin_definitions(&self) {
        // 先获取现有定义以检查是否需要保留参数
        let existing = self
            .dl
            .list_workflow_definitions()
            .await
            .unwrap_or_default();

        for definition in Self::builtin_definitions() {
            let mut parameters_json =
                serde_json::to_string(&definition.parameters).unwrap_or_else(|_| "{}".to_string());

            // 如果数据库中已经存在该 ID 的定义，则保留原有的参数配置，防止被内置默认值覆盖
            if let Some(record) = existing.iter().find(|r| r.id == definition.id) {
                if !record.parameters_json.is_empty() && record.parameters_json != "{}" {
                    parameters_json = record.parameters_json.clone();
                }
            }

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
            Ok(records) if !records.is_empty() => {
                records.into_iter().map(Self::from_record).collect()
            }
            _ => Self::builtin_definitions(),
        }
    }

    pub async fn grok_readiness(&self) -> GrokReadinessReport {
        let parameters = self
            .resolve_definition("grok_register_default")
            .await
            .map(|definition| definition.parameters)
            .unwrap_or_default();
        let mut checks = Vec::new();

        let resolved_domain = resolve_registration_domain(&self.dl, &parameters).await;
        match &resolved_domain {
            Ok(resolved) => checks.push(GrokReadinessCheck {
                id: "mail_domain",
                status: "pass",
                message: format!(
                    "收信域名 {} 已就绪（来源：{}）",
                    resolved.value, resolved.source
                ),
            }),
            Err(error) => checks.push(GrokReadinessCheck {
                id: "mail_domain",
                status: "fail",
                message: error.clone(),
            }),
        }

        let hub_secret_ready = env::var("HUB_SECRET")
            .ok()
            .is_some_and(|value| !value.trim().is_empty());
        checks.push(GrokReadinessCheck {
            id: "hub_secret",
            status: if hub_secret_ready { "pass" } else { "fail" },
            message: if hub_secret_ready {
                "Hub 邮件接入密钥已配置".to_string()
            } else {
                "Hub 未配置 HUB_SECRET，Cloudflare Worker 无法注入验证邮件".to_string()
            },
        });

        let cloudflare_public_url = self
            .dl
            .get_setting("cloudflare_public_url")
            .await
            .ok()
            .flatten();
        let public_hub_url = self.dl.get_setting("public_hub_url").await.ok().flatten();
        let public_url = trimmed_value(cloudflare_public_url.as_deref())
            .or_else(|| trimmed_value(public_hub_url.as_deref()));
        let public_url_ready = public_url.as_deref().is_some_and(|value| {
            reqwest::Url::parse(value).is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
            })
        });
        checks.push(GrokReadinessCheck {
            id: "public_hub",
            status: if public_url_ready { "pass" } else { "warn" },
            message: if public_url_ready {
                "Hub 公网入口已配置".to_string()
            } else {
                "未发现有效的 Hub 公网入口，请确认 Worker 的 PHANTOM_HUB_URL".to_string()
            },
        });

        if let Ok(resolved) = &resolved_domain {
            let zone_domain = self
                .dl
                .get_setting("cloudflare_zone_domain")
                .await
                .ok()
                .flatten()
                .and_then(|value| normalized_domain(Some(&value)));
            let routed_by_zone = zone_domain.as_deref().is_some_and(|zone| {
                resolved.value == zone || resolved.value.ends_with(&format!(".{zone}"))
            });
            checks.push(GrokReadinessCheck {
                id: "cloudflare_zone",
                status: if routed_by_zone { "pass" } else { "warn" },
                message: if routed_by_zone {
                    "收信域名与 Cloudflare 托管区域匹配".to_string()
                } else {
                    "收信域名与 cloudflare_zone_domain 不匹配，请确认 Catch-all 路由".to_string()
                },
            });

            let recent_mail = self
                .dl
                .get_emails(1, Some(&format!("@{}", resolved.value)), None)
                .await
                .ok()
                .and_then(|mut emails| emails.pop());
            let recent_mail_ready = recent_mail
                .as_ref()
                .is_some_and(|email| email.created_at >= Utc::now().timestamp() - 30 * 86_400);
            checks.push(GrokReadinessCheck {
                id: "mail_history",
                status: if recent_mail_ready { "pass" } else { "warn" },
                message: if recent_mail_ready {
                    "最近 30 天已有该域名邮件成功入库".to_string()
                } else {
                    "最近 30 天没有该域名的入库记录，建议先执行邮件中继测试".to_string()
                },
            });
        }

        if let Some(proxy_url) = trimmed_value(parameters.proxy_url.as_deref()) {
            let valid = reqwest::Proxy::all(&proxy_url).is_ok();
            checks.push(GrokReadinessCheck {
                id: "proxy",
                status: if valid { "pass" } else { "fail" },
                message: if valid {
                    "Grok 出口代理格式有效".to_string()
                } else {
                    "Grok 代理地址无效".to_string()
                },
            });
        } else {
            checks.push(GrokReadinessCheck {
                id: "proxy",
                status: "warn",
                message: "未配置 Grok 出口代理，受限网络环境可能被 xAI 拒绝".to_string(),
            });
        }

        let solver_mode = if trimmed_value(parameters.captcha_key.as_deref()).is_some() {
            checks.push(GrokReadinessCheck {
                id: "turnstile",
                status: "pass",
                message: "Turnstile 将优先使用 YesCaptcha".to_string(),
            });
            "yescaptcha"
        } else if let Some(solver_url) = trimmed_value(parameters.turnstile_solver_url.as_deref()) {
            let valid = crate::grok::register::validate_solver_url(&solver_url).is_ok();
            checks.push(GrokReadinessCheck {
                id: "turnstile",
                status: if valid { "pass" } else { "fail" },
                message: if valid {
                    "Turnstile 本地 Solver 地址有效".to_string()
                } else {
                    "Turnstile 本地 Solver 地址无效".to_string()
                },
            });
            "local_solver"
        } else {
            let chrome_ready = headless_chrome::browser::default_executable().is_ok();
            checks.push(GrokReadinessCheck {
                id: "turnstile",
                status: if chrome_ready { "warn" } else { "fail" },
                message: if chrome_ready {
                    "未配置外部 Solver，将使用 Chromium 回退，成功率取决于运行环境".to_string()
                } else {
                    "未配置外部 Solver，且运行环境中未检测到 Chrome/Chromium".to_string()
                },
            });
            "chromium_fallback"
        };

        let ready = checks.iter().all(|check| check.status != "fail");
        GrokReadinessReport {
            ready,
            domain: resolved_domain
                .as_ref()
                .ok()
                .map(|domain| domain.value.clone()),
            domain_source: resolved_domain.as_ref().ok().map(|domain| domain.source),
            solver_mode,
            checks,
        }
    }

    fn find_definition(
        definitions: &[WorkflowDefinition],
        workflow_id: &str,
    ) -> Option<WorkflowDefinition> {
        definitions
            .iter()
            .find(|definition| definition.id == workflow_id)
            .cloned()
    }

    async fn resolve_definition(&self, workflow_id: &str) -> Result<WorkflowDefinition, String> {
        let definitions = self.definitions().await;
        Self::find_definition(&definitions, workflow_id)
            .ok_or_else(|| format!("工作流不存在: {workflow_id}"))
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
        let permit = Arc::clone(&self.run_slots)
            .try_acquire_owned()
            .map_err(|_| "工作流并发已达上限，请稍后重试".to_string())?;
        let definition = self.resolve_definition(workflow_id).await?;
        let id = definition.id.clone();
        let run_id = Uuid::new_v4().to_string();

        let _ = dl
            .create_workflow_run(
                &run_id,
                &id,
                &definition.title,
                "running",
                "工作流已进入执行队列",
            )
            .await;

        let run_id_for_task = run_id.clone();
        let definition_for_task = definition;

        tokio::spawn(async move {
            let _permit = permit;
            let mut context = WorkflowRunContext {
                run_id: run_id_for_task.clone(),
                workflow_id: id.clone(),
                workflow_title: definition_for_task.title.to_string(),
                step_index: 0,
            };

            // 1. 发送开始任务日志
            Self::log_step(
                &hub,
                &dl,
                &mut context,
                "info",
                &format!("开始执行工作流: [{id}] / RUN_ID: {run_id_for_task}"),
            )
            .await;

            if let Err(message) = Self::validate_parameters(&definition_for_task) {
                Self::log_step(&hub, &dl, &mut context, "warn", &message).await;
                let _ = dl
                    .finish_workflow_run(&run_id_for_task, "warn", &message)
                    .await;
                return;
            }

            match definition_for_task.kind {
                WorkflowKind::AccountGenerate => {
                    match Self::simulate_account_gen(
                        &hub,
                        &dl,
                        &mut context,
                        &definition_for_task.parameters,
                    )
                    .await
                    {
                        Ok(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "success", &message)
                                .await;
                        }
                        Err(message) if message == "cancelled" => {
                            // 保持数据库中的 cancelled 状态，仅记录一条结束语
                            Self::log_step(&hub, &dl, &mut context, "info", "工作流已终止执行")
                                .await;
                        }
                        Err(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "warn", &message)
                                .await;
                        }
                    }
                }
                WorkflowKind::StatusReport => {
                    match Self::simulate_status_check(
                        &hub,
                        &dl,
                        &mut context,
                        &definition_for_task.parameters,
                    )
                    .await
                    {
                        Ok(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "success", &message)
                                .await;
                        }
                        Err(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "warn", &message)
                                .await;
                        }
                    }
                }
                WorkflowKind::DataCleanup => {
                    match Self::simulate_data_cleanup(
                        hub.clone(),
                        dl.clone(),
                        &mut context,
                        &definition_for_task.parameters,
                    )
                    .await
                    {
                        Ok(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "success", &message)
                                .await;
                        }
                        Err(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "warn", &message)
                                .await;
                        }
                    }
                }
                WorkflowKind::EnvironmentCheck => {
                    match Self::check_environment(
                        &hub,
                        &dl,
                        &mut context,
                        &definition_for_task.parameters,
                    )
                    .await
                    {
                        Ok(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "success", &message)
                                .await;
                        }
                        Err(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "warn", &message)
                                .await;
                        }
                    }
                }
                WorkflowKind::OpenAIRegister => {
                    match Self::openai_register_flow(
                        &hub,
                        &dl,
                        &mut context,
                        &definition_for_task.parameters,
                    )
                    .await
                    {
                        Ok(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "success", &message)
                                .await;
                        }
                        Err(message) if message == "cancelled" => {
                            Self::log_step(&hub, &dl, &mut context, "info", "工作流已终止执行")
                                .await;
                        }
                        Err(message) => {
                            Self::log_step(&hub, &dl, &mut context, "error", &message).await;
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "error", &message)
                                .await;
                        }
                    }
                }
                WorkflowKind::OpenAIRegisterBrowser => {
                    match Self::openai_browser_register_flow(
                        &hub,
                        &dl,
                        &mut context,
                        &definition_for_task.parameters,
                    )
                    .await
                    {
                        Ok(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "success", &message)
                                .await;
                        }
                        Err(message) if message == "cancelled" => {
                            Self::log_step(&hub, &dl, &mut context, "info", "工作流已终止执行")
                                .await;
                        }
                        Err(message) => {
                            Self::log_step(&hub, &dl, &mut context, "error", &message).await;
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "error", &message)
                                .await;
                        }
                    }
                }
                WorkflowKind::GrokRegister => {
                    match Self::grok_register_flow(
                        &hub,
                        &dl,
                        &mut context,
                        &definition_for_task.parameters,
                    )
                    .await
                    {
                        Ok(message) => {
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "success", &message)
                                .await;
                        }
                        Err(message) if message == "cancelled" => {
                            Self::log_step(&hub, &dl, &mut context, "info", "工作流已终止执行")
                                .await;
                        }
                        Err(message) => {
                            Self::log_step(&hub, &dl, &mut context, "error", &message).await;
                            let _ = dl
                                .finish_workflow_run(&run_id_for_task, "error", &message)
                                .await;
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
    ) -> Result<String, String> {
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

        Self::log_step(
            hub,
            dl,
            context,
            "info",
            &format!("正在通过 Catch-all 下发表单，目标批次: {generated_count} 个账户..."),
        )
        .await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;

        let mut created = 0usize;
        for index in 0..generated_count {
            // 实时检查是否已被用户手动停止
            if let Ok(current_status) = dl.get_workflow_run_status(&context.run_id).await {
                if current_status == "cancelled" {
                    Self::log_step(
                        hub,
                        dl,
                        context,
                        "warn",
                        "检测到用户终止指令，正在退出工作流...",
                    )
                    .await;
                    return Err("cancelled".to_string());
                }
            }

            let len = rand::thread_rng().gen_range(8..=12);
            let local_part: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(len)
                .map(|b| char::from(b).to_ascii_lowercase())
                .collect();

            let suffix = Uuid::new_v4().simple().to_string();
            let password = format!("Pwd{}_{}", Utc::now().timestamp() % 100000, &suffix[8..12]);
            let address = format!("{local_part}@{domain}");

            match dl
                .create_generated_account(
                    &context.run_id,
                    &address,
                    &password,
                    "ready",
                    parameters.account_type.as_deref(),
                    parameters.proxy_url.as_deref(),
                )
                .await
            {
                Ok(_) => {
                    created += 1;
                    if index < 3 {
                        Self::log_step(
                            hub,
                            dl,
                            context,
                            "info",
                            &format!("已生成账号产物: {address}"),
                        )
                        .await;
                    }
                }
                Err(e) => {
                    Self::log_step(
                        hub,
                        dl,
                        context,
                        "warn",
                        &format!("账号产物写入失败: {address} ({e})"),
                    )
                    .await;
                }
            }
        }

        Self::log_step(
            hub,
            dl,
            context,
            "success",
            &format!("账号产物写入完成: {created}/{generated_count}"),
        )
        .await;

        let message = format!(
            "工作流执行成功: {} / DONE / GENERATED: {}",
            context.workflow_id, created
        );
        Ok(message)
    }

    fn from_record(record: WorkflowDefinitionRecord) -> WorkflowDefinition {
        let is_builtin = Self::builtin_ids()
            .iter()
            .any(|builtin_id| builtin_id == &record.id.as_str());
        let parameters =
            serde_json::from_str::<WorkflowParameters>(&record.parameters_json).unwrap_or_default();
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
                    return Err(
                        "负载报告参数无效 / report_window_hours 必须在 1..=168 之间".to_string()
                    );
                }
                Ok(())
            }
            WorkflowKind::GrokRegister => {
                let batch_size = definition.parameters.batch_size.unwrap_or(1);
                let concurrency = definition.parameters.concurrency.unwrap_or(1);
                let timeout = definition
                    .parameters
                    .registration_timeout_secs
                    .unwrap_or(180);
                if !(1..=50).contains(&batch_size) {
                    return Err("Grok 注册参数无效 / batch_size 必须在 1..=50 之间".to_string());
                }
                if !(1..=10).contains(&concurrency) {
                    return Err("Grok 注册参数无效 / concurrency 必须在 1..=10 之间".to_string());
                }
                if !(60..=600).contains(&timeout) {
                    return Err(
                        "Grok 注册参数无效 / registration_timeout_secs 必须在 60..=600 之间"
                            .to_string(),
                    );
                }
                if let Some(domain) = trimmed_value(definition.parameters.account_domain.as_deref())
                {
                    let domain = normalized_domain(Some(&domain)).unwrap_or_default();
                    validate_public_mail_domain(&domain)?;
                }
                if let Some(proxy_url) = trimmed_value(definition.parameters.proxy_url.as_deref()) {
                    reqwest::Proxy::all(&proxy_url)
                        .map_err(|error| format!("Grok 代理地址无效: {error}"))?;
                }
                if let Some(solver_url) =
                    trimmed_value(definition.parameters.turnstile_solver_url.as_deref())
                {
                    crate::grok::register::validate_solver_url(&solver_url)?;
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

        Self::log_step(
            &hub,
            &dl,
            context,
            "info",
            &format!("正在评估数据湖过期指纹记录，保留天数: {resolved}..."),
        )
        .await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        match dl.cleanup_emails(resolved).await {
            Ok(count) => {
                let message = format!("数据湖自动清理完成: 成功回收 {count} 条过期记录");
                Self::log_step(&hub, &dl, context, "success", &message).await;
                Ok(message)
            }
            Err(e) => {
                let message = format!("数据清理执行异常: {e:?}");
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
        Self::log_step(
            hub,
            dl,
            context,
            "info",
            &format!("正在对 SQLite 数据湖进行健康度快照，统计窗口: {report_window_hours}h..."),
        )
        .await;
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;

        let stats = dl
            .get_dashboard_stats()
            .await
            .map_err(|error| format!("统计查询失败: {error:?}"))?;
        let coverage = if stats.total_emails == 0 {
            0
        } else {
            (stats.code_emails * 100) / stats.total_emails
        };
        let summary = format!(
            "负载报告就绪: [总邮件: {}] [24h新增: {}] [验证码覆盖: {}%] [活跃Webhook: {}]",
            stats.total_emails, stats.recent_emails_24h, coverage, stats.active_webhooks
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

        let hub_secret = env::var("HUB_SECRET")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let public_hub_url = dl
            .get_setting("public_hub_url")
            .await
            .ok()
            .flatten()
            .filter(|value| !value.trim().is_empty());
        let webhook_url = dl
            .get_setting("webhook_url")
            .await
            .ok()
            .flatten()
            .filter(|value| !value.trim().is_empty());
        let active_hooks = dl
            .get_active_webhooks()
            .await
            .map(|hooks| hooks.len())
            .unwrap_or(0);
        let require_public_hub_url = parameters.require_public_hub_url.unwrap_or(true);
        let require_webhook = parameters.require_webhook.unwrap_or(false);

        if hub_secret.is_some() {
            Self::log_step(hub, dl, context, "success", "已配置机器通信密钥 HUB_SECRET").await;
        } else {
            Self::log_step(
                hub,
                dl,
                context,
                "warn",
                "未配置 HUB_SECRET，Worker 邮件接入处于禁用状态",
            )
            .await;
        }

        if public_hub_url.is_some() {
            Self::log_step(hub, dl, context, "success", "已登记公网访问地址").await;
        } else {
            Self::log_step(
                hub,
                dl,
                context,
                "warn",
                "尚未登记公网访问地址，Worker 无法从公网访问中枢",
            )
            .await;
        }

        if webhook_url.is_some() || active_hooks > 0 {
            Self::log_step(
                hub,
                dl,
                context,
                "success",
                &format!("Webhook 配置可用，当前活跃数: {active_hooks}"),
            )
            .await;
        } else {
            Self::log_step(hub, dl, context, "warn", "尚未配置可用的 Webhook 回调地址").await;
        }

        let public_hub_ok = public_hub_url.is_some() || !require_public_hub_url;
        let webhook_ok = webhook_url.is_some() || active_hooks > 0 || !require_webhook;
        let secrets_ok = hub_secret.is_some();

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

    async fn openai_register_flow(
        hub: &Arc<StreamHub>,
        dl: &Arc<DataLake>,
        context: &mut WorkflowRunContext,
        parameters: &WorkflowParameters,
    ) -> Result<String, String> {
        let batch_size = parameters.batch_size.unwrap_or(1).clamp(1, 50);
        let proxy_url = parameters.proxy_url.clone();
        let configured_domain = dl.get_setting("account_domain").await.ok().flatten();
        let domain = parameters
            .account_domain
            .clone()
            .or(configured_domain)
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "phantom.local".to_string());

        Self::log_step(
            hub,
            dl,
            context,
            "info",
            &format!("正在初始化 OpenAI 协议套件 | 目标批次: {batch_size} | 域名: {domain}"),
        )
        .await;

        if proxy_url
            .as_ref()
            .is_some_and(|proxy| !proxy.trim().is_empty())
        {
            Self::log_step(hub, dl, context, "info", "代理服务器已配置（详情已隐藏）").await;
        }

        let mut success_count = 0usize;
        let mut fail_count = 0usize;

        for index in 0..batch_size {
            // 实时检查是否已被用户手动停止
            if let Ok(current_status) = dl.get_workflow_run_status(&context.run_id).await {
                if current_status == "cancelled" {
                    Self::log_step(
                        hub,
                        dl,
                        context,
                        "warn",
                        "检测到用户终止指令，正在退出工作流...",
                    )
                    .await;
                    return Err("cancelled".to_string());
                }
            }

            // 生成随机邮箱和密码
            let len = rand::thread_rng().gen_range(8..=12);
            let local_part: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(len)
                .map(|b| char::from(b).to_ascii_lowercase())
                .collect();
            let email = format!("{local_part}@{domain}");
            let password: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(12)
                .map(char::from)
                .collect();
            let device_id = crate::openai::oauth::generate_device_id();

            Self::log_step(
                hub,
                dl,
                context,
                "info",
                &format!(
                    "[{}/{}] 开始注册: {} | 密令: ******",
                    index + 1,
                    batch_size,
                    email
                ),
            )
            .await;

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();

            let register_context = crate::openai::register::RegisterContext {
                email: email.clone(),
                password: password.clone(),
                device_id: device_id.clone(),
                proxy_url: proxy_url.clone(),
                captcha_key: parameters.captcha_key.clone(),
                sms_key: parameters.sms_key.clone(),
                full_name: parameters.full_name.clone(),
                age: parameters.age,
                headless: true, // 协议模式下强行设定为 true
                run_id: context.run_id.clone(),
                step_callback: Some(Box::new(move |level, msg| {
                    let _ = tx.send((level.to_string(), msg.to_string()));
                })),
                oauth_authorize_url: None,
                oauth_code_verifier: None,
                oauth_platform: None,
            };

            let dl_clone = Arc::clone(dl);

            // 运行注册核心逻辑 (Spawn)
            let register_task = tokio::spawn(async move {
                crate::openai::register::execute_registration(&dl_clone, &register_context).await
            });

            // 监听回调并更新日志
            while let Some((level, msg)) = rx.recv().await {
                Self::log_step(hub, dl, context, &level, &msg).await;
            }

            // 等待最终结果
            match register_task.await {
                Ok(Ok(result)) => {
                    Self::log_step(
                        hub,
                        dl,
                        context,
                        "success",
                        &format!("[{}/{}] 注册引擎执行成功: {}", index + 1, batch_size, email),
                    )
                    .await;

                    if let Ok(account_id) = dl
                        .create_generated_account(
                            &context.run_id,
                            &result.email,
                            &result.password,
                            "openai_registered",
                            parameters.account_type.as_deref(),
                            parameters.proxy_url.as_deref(),
                        )
                        .await
                    {
                        let id_token_opt = result.id_token.as_deref();
                        let auth_info = id_token_opt
                            .map(|idt| crate::openai::oauth::extract_auth_info_from_jwt(idt));
                        let chatgpt_account_id = auth_info
                            .as_ref()
                            .and_then(|info| info.chatgpt_account_id.as_deref());
                        let chatgpt_user_id = auth_info
                            .as_ref()
                            .and_then(|info| info.chatgpt_user_id.as_deref());
                        let organization_id = auth_info
                            .as_ref()
                            .and_then(|info| info.organization_id.as_deref());
                        let plan_type = auth_info
                            .as_ref()
                            .and_then(|info| info.plan_type.as_deref());
                        let expires_in = crate::openai::oauth::DEFAULT_OAUTH_EXPIRES_IN;
                        let token_version = crate::openai::oauth::DEFAULT_OAUTH_TOKEN_VERSION;
                        let oauth_credentials = build_oauth_credentials(
                            &result.email,
                            &result,
                            chatgpt_account_id,
                            chatgpt_user_id,
                            organization_id,
                            plan_type,
                            expires_in,
                            token_version,
                        );
                        let access_token = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.access_token.as_str(),
                        );
                        let refresh_token = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.refresh_token.as_str(),
                        );
                        let id_token = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.id_token.as_str(),
                        );
                        let chatgpt_account_id = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.chatgpt_account_id.as_str(),
                        );
                        let chatgpt_user_id = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.chatgpt_user_id.as_str(),
                        );
                        let organization_id = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.organization_id.as_str(),
                        );

                        let _ = dl
                            .update_account_tokens(
                                &account_id,
                                access_token,
                                refresh_token,
                                result.session_token.as_deref(),
                                Some(&result.device_id),
                                result.workspace_id.as_deref(),
                                id_token,
                                chatgpt_account_id,
                                chatgpt_user_id,
                                organization_id,
                                Some(oauth_credentials.plan_type.as_str()),
                                Some(oauth_credentials.expires_in),
                                Some(oauth_credentials.token_version),
                                oauth_credentials.json.as_deref(),
                            )
                            .await;

                        // 既然注册已完成且有了 Session Token，立刻触发一次账户状态校验与 Token 自动补全
                        if result.session_token.is_some() {
                            Self::log_step(
                                hub,
                                dl,
                                context,
                                "info",
                                "🔑 检测到 Session Token，正在触发后端自动双重校验与 Access Token 换取...",
                            )
                            .await;
                            match crate::openai::checker::check_account_status(
                                Arc::clone(dl),
                                &account_id,
                            )
                            .await
                            {
                                Ok(status) => {
                                    Self::log_step(
                                        hub,
                                        dl,
                                        context,
                                        "success",
                                        &format!("✅ 自动双重校验完成，当前账户状态: {status}"),
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    Self::log_step(
                                        hub,
                                        dl,
                                        context,
                                        "warn",
                                        &format!("⚠️ 自动双重校验及 Token 补全失败: {e}"),
                                    )
                                    .await;
                                }
                            }
                        }

                        // 获取 CPA 分发参数 (参数优先，全局设置/Codex 授权令牌兜底)
                        let mut final_cpa_url = parameters.cpa_url.clone().unwrap_or_default();
                        let mut final_cpa_key = parameters.cpa_key.clone().unwrap_or_default();

                        if final_cpa_url.trim().is_empty() {
                            if let Ok(Some(u)) = dl.get_setting("cpa_url").await {
                                final_cpa_url = u;
                            }
                        }
                        if final_cpa_key.trim().is_empty() {
                            if let Ok(Some(k)) = dl.get_setting("cpa_key").await {
                                final_cpa_key = k;
                            }
                        }
                        if final_cpa_key.trim().is_empty() {
                            if let Ok(Some(auth_json)) = dl.get_setting("cpa_auth_json").await {
                                if let Ok(auth_data) = serde_json::from_str::<
                                    crate::openai::oauth::CodexAuthData,
                                >(&auth_json)
                                {
                                    final_cpa_key = auth_data.access_token;
                                }
                            }
                        }

                        if !final_cpa_url.trim().is_empty() && !final_cpa_key.trim().is_empty() {
                            let mut cpa_url_endpoint = final_cpa_url.trim().to_string();
                            if !cpa_url_endpoint.contains("/v0/")
                                && !cpa_url_endpoint.contains("/api/")
                            {
                                cpa_url_endpoint = format!(
                                    "{}/v0/management/auth-files",
                                    cpa_url_endpoint.trim_end_matches('/')
                                );
                            }

                            Self::log_step(
                                hub,
                                dl,
                                context,
                                "info",
                                &format!(
                                    "[{}/{}] 准备推送账号至分发平台...",
                                    index + 1,
                                    batch_size
                                ),
                            )
                            .await;

                            let client = match crate::routes::build_ssrf_safe_client(
                                &cpa_url_endpoint,
                            )
                            .await
                            {
                                Ok((_, client)) => client,
                                Err(error) => {
                                    Self::log_step(
                                        hub,
                                        dl,
                                        context,
                                        "error",
                                        &format!("CPA 地址安全校验失败: {error}"),
                                    )
                                    .await;
                                    continue;
                                }
                            };
                            if let Ok(Some(acc)) = dl.get_generated_account(&account_id).await {
                                let payload = crate::exporter::AccountExporter::transform(
                                    &acc,
                                    crate::exporter::ExportFormat::Cpa,
                                );
                                match crate::uploader::upload_account_multipart(
                                    &client,
                                    &cpa_url_endpoint,
                                    final_cpa_key.trim(),
                                    payload,
                                )
                                .await
                                {
                                    Ok(_) => {
                                        let _ = dl
                                            .update_account_upload_status(&account_id, "success")
                                            .await;
                                        Self::log_step(
                                            hub,
                                            dl,
                                            context,
                                            "success",
                                            &format!("[{}/{}] 账号分发成功", index + 1, batch_size),
                                        )
                                        .await;
                                    }
                                    Err(e) => {
                                        let _ = dl
                                            .update_account_upload_status(&account_id, "failed")
                                            .await;
                                        Self::log_step(
                                            hub,
                                            dl,
                                            context,
                                            "error",
                                            &format!(
                                                "[{}/{}] 账号分发失败: {}",
                                                index + 1,
                                                batch_size,
                                                e
                                            ),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                Self::log_step(
                                    hub,
                                    dl,
                                    context,
                                    "error",
                                    &format!(
                                        "[{}/{}] 账号分发失败：无法从数据库读取生成的账号详情",
                                        index + 1,
                                        batch_size
                                    ),
                                )
                                .await;
                            }
                        }
                    }
                    success_count += 1;
                }
                Ok(Err(err)) => {
                    Self::log_step(
                        hub,
                        dl,
                        context,
                        "error",
                        &format!("[{}/{}] 注册引擎执行失败: {}", index + 1, batch_size, err),
                    )
                    .await;
                    let _ = dl
                        .create_generated_account(
                            &context.run_id,
                            &email,
                            &password,
                            "register_failed",
                            parameters.account_type.as_deref(),
                            parameters.proxy_url.as_deref(),
                        )
                        .await;
                    fail_count += 1;
                }
                Err(err) => {
                    Self::log_step(
                        hub,
                        dl,
                        context,
                        "error",
                        &format!("[{}/{}] 注册引擎崩溃: {:?}", index + 1, batch_size, err),
                    )
                    .await;
                    let _ = dl
                        .create_generated_account(
                            &context.run_id,
                            &email,
                            &password,
                            "register_failed",
                            parameters.account_type.as_deref(),
                            parameters.proxy_url.as_deref(),
                        )
                        .await;
                    fail_count += 1;
                }
            }

            // 批量间隔
            if index + 1 < batch_size {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
        }

        let summary = format!(
            "OpenAI 批量注册完成 | 成功: {success_count} | 失败: {fail_count} | 总计: {batch_size}"
        );
        Self::log_step(hub, dl, context, "success", &summary).await;

        if success_count > 0 {
            Ok(summary)
        } else {
            Err(format!("全部 {batch_size} 个账号注册失败"))
        }
    }

    async fn grok_register_flow(
        hub: &Arc<StreamHub>,
        dl: &Arc<DataLake>,
        context: &mut WorkflowRunContext,
        parameters: &WorkflowParameters,
    ) -> Result<String, String> {
        let batch_size = parameters.batch_size.unwrap_or(1).clamp(1, 50);
        let concurrency = parameters
            .concurrency
            .unwrap_or(1)
            .clamp(1, 10)
            .min(batch_size);
        let resolved_domain = resolve_registration_domain(dl, parameters).await?;
        let domain = resolved_domain.value;
        if env::var("HUB_SECRET")
            .ok()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(
                "Grok 注册前必须配置 HUB_SECRET，否则 Cloudflare Worker 无法注入验证邮件"
                    .to_string(),
            );
        }

        let timeout_secs = parameters.registration_timeout_secs.unwrap_or(180);
        let solver_mode = if parameters
            .captcha_key
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            "YesCaptcha"
        } else if parameters
            .turnstile_solver_url
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            "Local Solver"
        } else {
            "Chromium fallback"
        };
        Self::log_step(
            hub,
            dl,
            context,
            "info",
            &format!(
                "正在初始化 Grok 注册批次 | 数量: {batch_size} | 并发: {concurrency} | 域名: {domain} | 来源: {} | Turnstile: {solver_mode}",
                resolved_domain.source
            ),
        )
        .await;

        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
        let mut jobs = Vec::with_capacity(batch_size);
        for index in 0..batch_size {
            let local_len = rand::thread_rng().gen_range(10..=14);
            let local_part: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(local_len)
                .map(|value| char::from(value).to_ascii_lowercase())
                .collect();
            let email = format!("{local_part}@{domain}");
            let password = crate::grok::register::generate_password();
            let tx = event_tx.clone();
            let prefix = format!("[{}/{}] {}", index + 1, batch_size, email);
            let callback: crate::grok::register::StepCallback = Arc::new(move |level, message| {
                let _ = tx.send((level.to_string(), format!("{prefix} | {message}")));
            });
            jobs.push(crate::grok::register::GrokRegisterContext {
                email,
                password,
                proxy_url: parameters.proxy_url.clone(),
                captcha_key: parameters.captcha_key.clone(),
                turnstile_solver_url: parameters.turnstile_solver_url.clone(),
                headless: parameters.headless.unwrap_or(true),
                timeout_secs,
                run_id: context.run_id.clone(),
                step_callback: Some(callback),
            });
        }
        drop(event_tx);

        let dl_for_jobs = Arc::clone(dl);
        let tasks = futures_util::stream::iter(jobs.into_iter().enumerate().map(
            move |(index, register_context)| {
                let dl = Arc::clone(&dl_for_jobs);
                async move {
                    let email = register_context.email.clone();
                    let result =
                        crate::grok::register::execute_registration(&dl, &register_context).await;
                    (index, email, result)
                }
            },
        ))
        .buffer_unordered(concurrency);
        futures_util::pin_mut!(tasks);

        let mut success_count = 0usize;
        let mut fail_count = 0usize;
        let mut cancelled = false;

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    if let Some((level, message)) = event {
                        Self::log_step(hub, dl, context, &level, &message).await;
                    }
                }
                result = tasks.next() => {
                    let Some((index, email, result)) = result else {
                        while let Ok((level, message)) = event_rx.try_recv() {
                            Self::log_step(hub, dl, context, &level, &message).await;
                        }
                        break;
                    };
                    match result {
                        Ok(result) => {
                            match dl
                                .create_generated_account(
                                    &context.run_id,
                                    &result.email,
                                    &result.password,
                                    "grok_registered",
                                    parameters.account_type.as_deref().or(Some("grok_free")),
                                    parameters.proxy_url.as_deref(),
                                )
                                .await
                            {
                                Ok(account_id) => {
                                    if let Err(error) = dl
                                        .update_account_tokens(
                                            &account_id,
                                            None,
                                            None,
                                            Some(&result.sso),
                                            None,
                                            None,
                                            None,
                                            None,
                                            None,
                                            None,
                                            Some("grok_free"),
                                            None,
                                            Some(1),
                                            None,
                                        )
                                        .await
                                    {
                                        fail_count += 1;
                                        Self::log_step(
                                            hub,
                                            dl,
                                            context,
                                            "error",
                                            &format!(
                                                "[{}/{}] {} | SSO 凭证入库失败: {error}",
                                                index + 1,
                                                batch_size,
                                                email
                                            ),
                                        )
                                        .await;
                                        continue;
                                    }
                                    success_count += 1;
                                    Self::log_step(
                                        hub,
                                        dl,
                                        context,
                                        "success",
                                        &format!(
                                            "[{}/{}] {} | Grok 账号与 SSO 已入库",
                                            index + 1,
                                            batch_size,
                                            email
                                        ),
                                    )
                                    .await;
                                }
                                Err(error) => {
                                    fail_count += 1;
                                    Self::log_step(
                                        hub,
                                        dl,
                                        context,
                                        "error",
                                        &format!(
                                            "[{}/{}] {} | 账号入库失败: {error}",
                                            index + 1,
                                            batch_size,
                                            email
                                        ),
                                    )
                                    .await;
                                }
                            }
                        }
                        Err(error) if error == "cancelled" => {
                            cancelled = true;
                        }
                        Err(error) => {
                            fail_count += 1;
                            Self::log_step(
                                hub,
                                dl,
                                context,
                                "error",
                                &format!(
                                    "[{}/{}] {} | Grok 注册失败: {error}",
                                    index + 1,
                                    batch_size,
                                    email
                                ),
                            )
                            .await;
                        }
                    }
                }
            }
        }

        if cancelled {
            return Err("cancelled".to_string());
        }

        let summary = format!(
            "Grok 自动注册批次完成 | 成功: {success_count} | 失败: {fail_count} | 总计: {batch_size}"
        );
        Self::log_step(
            hub,
            dl,
            context,
            if success_count > 0 {
                "success"
            } else {
                "error"
            },
            &summary,
        )
        .await;
        if success_count > 0 {
            Ok(summary)
        } else {
            Err(summary)
        }
    }

    async fn openai_browser_register_flow(
        hub: &Arc<StreamHub>,
        dl: &Arc<DataLake>,
        context: &mut WorkflowRunContext,
        parameters: &WorkflowParameters,
    ) -> Result<String, String> {
        let batch_size = parameters.batch_size.unwrap_or(1);
        let mut success_count = 0;
        let mut fail_count = 0;

        for index in 0..batch_size {
            // 实时检查是否已被用户手动停止
            if let Ok(current_status) = dl.get_workflow_run_status(&context.run_id).await {
                if current_status == "cancelled" {
                    Self::log_step(
                        hub,
                        dl,
                        context,
                        "warn",
                        "检测到用户终止指令，正在退出工作流...",
                    )
                    .await;
                    return Err("cancelled".to_string());
                }
            }

            Self::log_step(
                hub,
                dl,
                context,
                "info",
                &format!("[{}/{}] 正在初始化浏览器仿真环境...", index + 1, batch_size),
            )
            .await;

            let domain = dl
                .get_setting("account_domain")
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "phantom.local".to_string());
            let len = rand::thread_rng().gen_range(8..=12);
            let local_part: String = rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(len)
                .map(|b| char::from(b).to_ascii_lowercase())
                .collect();
            let email = format!("{local_part}@{domain}");
            let password: String = rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(12)
                .map(char::from)
                .collect();

            Self::log_step(
                hub,
                dl,
                context,
                "info",
                &format!("🚀 准备注册: {email} | 密码: ******"),
            )
            .await;

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();

            let register_ctx = crate::openai::register::RegisterContext {
                email: email.clone(),
                password: password.clone(),
                device_id: crate::openai::oauth::generate_device_id(),
                proxy_url: parameters.proxy_url.clone(),
                captcha_key: parameters.captcha_key.clone(),
                sms_key: parameters.sms_key.clone(),
                full_name: parameters.full_name.clone(),
                age: parameters.age,
                headless: parameters.headless.unwrap_or(true),
                run_id: context.run_id.clone(),
                step_callback: Some(Box::new(move |level, msg| {
                    let _ = tx.send((level.to_string(), msg.to_string()));
                })),
                oauth_authorize_url: parameters.oauth_authorize_url.clone(),
                oauth_code_verifier: parameters.oauth_code_verifier.clone(),
                oauth_platform: parameters.oauth_platform.clone(),
            };

            let driver =
                crate::openai::browser_driver::BrowserDriver::new(register_ctx, dl.clone());

            // 运行驱动
            let driver_task = tokio::spawn(async move { driver.run().await });

            // 监听回调
            while let Some((level, msg)) = rx.recv().await {
                Self::log_step(hub, dl, context, &level, &msg).await;
            }

            match driver_task.await {
                Ok(Ok(result)) => {
                    success_count += 1;
                    if let Ok(account_id) = dl
                        .create_generated_account(
                            &context.run_id,
                            &result.email,
                            &result.password,
                            "openai_registered",
                            parameters.account_type.as_deref(),
                            parameters.proxy_url.as_deref(),
                        )
                        .await
                    {
                        let id_token_opt = result.id_token.as_deref();
                        let auth_info = id_token_opt
                            .map(|idt| crate::openai::oauth::extract_auth_info_from_jwt(idt));
                        let chatgpt_account_id = auth_info
                            .as_ref()
                            .and_then(|info| info.chatgpt_account_id.as_deref());
                        let chatgpt_user_id = auth_info
                            .as_ref()
                            .and_then(|info| info.chatgpt_user_id.as_deref());
                        let organization_id = auth_info
                            .as_ref()
                            .and_then(|info| info.organization_id.as_deref());
                        let plan_type = auth_info
                            .as_ref()
                            .and_then(|info| info.plan_type.as_deref());
                        let expires_in = crate::openai::oauth::DEFAULT_OAUTH_EXPIRES_IN;
                        let token_version = crate::openai::oauth::DEFAULT_OAUTH_TOKEN_VERSION;
                        let oauth_credentials = build_oauth_credentials(
                            &result.email,
                            &result,
                            chatgpt_account_id,
                            chatgpt_user_id,
                            organization_id,
                            plan_type,
                            expires_in,
                            token_version,
                        );
                        let access_token = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.access_token.as_str(),
                        );
                        let refresh_token = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.refresh_token.as_str(),
                        );
                        let id_token = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.id_token.as_str(),
                        );
                        let chatgpt_account_id = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.chatgpt_account_id.as_str(),
                        );
                        let chatgpt_user_id = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.chatgpt_user_id.as_str(),
                        );
                        let organization_id = crate::openai::oauth::parse_non_empty(
                            oauth_credentials.organization_id.as_str(),
                        );

                        let _ = dl
                            .update_account_tokens(
                                &account_id,
                                access_token,
                                refresh_token,
                                result.session_token.as_deref(),
                                Some(&result.device_id),
                                result.workspace_id.as_deref(),
                                id_token,
                                chatgpt_account_id,
                                chatgpt_user_id,
                                organization_id,
                                Some(oauth_credentials.plan_type.as_str()),
                                Some(oauth_credentials.expires_in),
                                Some(oauth_credentials.token_version),
                                oauth_credentials.json.as_deref(),
                            )
                            .await;
                        Self::log_step(
                            hub,
                            dl,
                            context,
                            "success",
                            &format!("✅ 账号及其凭证已保存至数据库: {email}"),
                        )
                        .await;

                        // 既然注册已完成且有了 Session Token，立刻触发一次账户状态校验与 Token 自动补全
                        if result.session_token.is_some() {
                            Self::log_step(
                                hub,
                                dl,
                                context,
                                "info",
                                "🔑 检测到 Session Token，正在触发后端自动双重校验与 Access Token 换取...",
                            )
                            .await;
                            match crate::openai::checker::check_account_status(
                                Arc::clone(dl),
                                &account_id,
                            )
                            .await
                            {
                                Ok(status) => {
                                    Self::log_step(
                                        hub,
                                        dl,
                                        context,
                                        "success",
                                        &format!("✅ 自动双重校验完成，当前账户状态: {status}"),
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    Self::log_step(
                                        hub,
                                        dl,
                                        context,
                                        "warn",
                                        &format!("⚠️ 自动双重校验及 Token 补全失败: {e}"),
                                    )
                                    .await;
                                }
                            }
                        }

                        // 获取 CPA 分发参数 (参数优先，全局设置/Codex 授权令牌兜底)
                        let mut final_cpa_url = parameters.cpa_url.clone().unwrap_or_default();
                        let mut final_cpa_key = parameters.cpa_key.clone().unwrap_or_default();

                        if final_cpa_url.trim().is_empty() {
                            if let Ok(Some(u)) = dl.get_setting("cpa_url").await {
                                final_cpa_url = u;
                            }
                        }
                        if final_cpa_key.trim().is_empty() {
                            if let Ok(Some(k)) = dl.get_setting("cpa_key").await {
                                final_cpa_key = k;
                            }
                        }
                        if final_cpa_key.trim().is_empty() {
                            if let Ok(Some(auth_json)) = dl.get_setting("cpa_auth_json").await {
                                if let Ok(auth_data) = serde_json::from_str::<
                                    crate::openai::oauth::CodexAuthData,
                                >(&auth_json)
                                {
                                    final_cpa_key = auth_data.access_token;
                                }
                            }
                        }

                        if !final_cpa_url.trim().is_empty() && !final_cpa_key.trim().is_empty() {
                            let mut cpa_url_endpoint = final_cpa_url.trim().to_string();
                            if !cpa_url_endpoint.contains("/v0/")
                                && !cpa_url_endpoint.contains("/api/")
                            {
                                cpa_url_endpoint = format!(
                                    "{}/v0/management/auth-files",
                                    cpa_url_endpoint.trim_end_matches('/')
                                );
                            }

                            Self::log_step(
                                hub,
                                dl,
                                context,
                                "info",
                                &format!(
                                    "[{}/{}] 准备推送账号至分发平台...",
                                    index + 1,
                                    batch_size
                                ),
                            )
                            .await;

                            let client = match crate::routes::build_ssrf_safe_client(
                                &cpa_url_endpoint,
                            )
                            .await
                            {
                                Ok((_, client)) => client,
                                Err(error) => {
                                    Self::log_step(
                                        hub,
                                        dl,
                                        context,
                                        "error",
                                        &format!("CPA 地址安全校验失败: {error}"),
                                    )
                                    .await;
                                    continue;
                                }
                            };
                            if let Ok(Some(acc)) = dl.get_generated_account(&account_id).await {
                                let payload = crate::exporter::AccountExporter::transform(
                                    &acc,
                                    crate::exporter::ExportFormat::Cpa,
                                );
                                match crate::uploader::upload_account_multipart(
                                    &client,
                                    &cpa_url_endpoint,
                                    final_cpa_key.trim(),
                                    payload,
                                )
                                .await
                                {
                                    Ok(_) => {
                                        let _ = dl
                                            .update_account_upload_status(&account_id, "success")
                                            .await;
                                        Self::log_step(
                                            hub,
                                            dl,
                                            context,
                                            "success",
                                            &format!("[{}/{}] 账号分发成功", index + 1, batch_size),
                                        )
                                        .await;
                                    }
                                    Err(e) => {
                                        let _ = dl
                                            .update_account_upload_status(&account_id, "failed")
                                            .await;
                                        Self::log_step(
                                            hub,
                                            dl,
                                            context,
                                            "error",
                                            &format!(
                                                "[{}/{}] 账号分发失败: {}",
                                                index + 1,
                                                batch_size,
                                                e
                                            ),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                Self::log_step(
                                    hub,
                                    dl,
                                    context,
                                    "error",
                                    &format!(
                                        "[{}/{}] 账号分发失败：无法从数据库读取生成的账号详情",
                                        index + 1,
                                        batch_size
                                    ),
                                )
                                .await;
                            }
                        }
                    } else {
                        Self::log_step(
                            hub,
                            dl,
                            context,
                            "error",
                            &format!("账号入库失败: {email}"),
                        )
                        .await;
                    }
                }
                Ok(Err(e)) => {
                    fail_count += 1;
                    Self::log_step(hub, dl, context, "error", &format!("单次注册失败: {e}")).await;
                }
                Err(e) => {
                    fail_count += 1;
                    Self::log_step(hub, dl, context, "error", &format!("任务意外崩溃: {e:?}"))
                        .await;
                }
            }

            // 批量间隔，防止操作过快被检测
            if index + 1 < batch_size {
                let sleep_secs = rand::thread_rng().gen_range(5..15);
                Self::log_step(
                    hub,
                    dl,
                    context,
                    "info",
                    &format!("💤 等待 {sleep_secs} 秒后开始下一个任务..."),
                )
                .await;
                tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
            }
        }

        let summary = format!(
            "OpenAI 浏览器模拟注册批量完成 | 成功: {success_count} | 失败: {fail_count} | 总计: {batch_size}"
        );
        Self::log_step(hub, dl, context, "success", &summary).await;

        if success_count > 0 {
            Ok(summary)
        } else {
            Err(format!("全部 {batch_size} 个浏览器注册任务均失败"))
        }
    }

    async fn log_step(
        hub: &Arc<StreamHub>,
        dl: &Arc<DataLake>,
        context: &mut WorkflowRunContext,
        level: &str,
        msg: &str,
    ) {
        context.step_index += 1;
        let msg = crate::utils::redact_log_message(msg);
        let _ = dl
            .add_workflow_step(&context.run_id, context.step_index, level, &msg)
            .await;

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

fn build_oauth_credentials(
    email: &str,
    result: &crate::openai::register::RegisterResult,
    chatgpt_account_id: Option<&str>,
    chatgpt_user_id: Option<&str>,
    organization_id: Option<&str>,
    plan_type: Option<&str>,
    expires_in: i64,
    token_version: i64,
) -> crate::openai::oauth::BuiltOAuthCredentials {
    crate::openai::oauth::build_oauth_credentials(crate::openai::oauth::OAuthCredentialInput {
        email,
        access_token: result.access_token.as_deref(),
        refresh_token: result.refresh_token.as_deref(),
        id_token: result.id_token.as_deref(),
        workspace_id: result.workspace_id.as_deref(),
        chatgpt_account_id,
        chatgpt_user_id,
        organization_id,
        plan_type,
        expires_in: Some(expires_in),
        token_version: Some(token_version),
        stored_credentials: None,
    })
}

impl WorkflowKind {
    pub fn as_storage(&self) -> &'static str {
        match self {
            WorkflowKind::AccountGenerate => "account_generate",
            WorkflowKind::DataCleanup => "data_cleanup",
            WorkflowKind::StatusReport => "status_report",
            WorkflowKind::EnvironmentCheck => "environment_check",
            WorkflowKind::OpenAIRegister => "openai_register",
            WorkflowKind::OpenAIRegisterBrowser => "openai_register_browser",
            WorkflowKind::GrokRegister => "grok_register",
        }
    }

    pub fn from_storage(value: &str) -> Self {
        match value {
            "data_cleanup" => WorkflowKind::DataCleanup,
            "status_report" => WorkflowKind::StatusReport,
            "environment_check" => WorkflowKind::EnvironmentCheck,
            "openai_register" => WorkflowKind::OpenAIRegister,
            "openai_register_browser" => WorkflowKind::OpenAIRegisterBrowser,
            "grok_register" => WorkflowKind::GrokRegister,
            _ => WorkflowKind::AccountGenerate,
        }
    }

    pub fn try_from_storage(value: &str) -> Result<Self, String> {
        match value {
            "account_generate" => Ok(WorkflowKind::AccountGenerate),
            "data_cleanup" => Ok(WorkflowKind::DataCleanup),
            "status_report" => Ok(WorkflowKind::StatusReport),
            "environment_check" => Ok(WorkflowKind::EnvironmentCheck),
            "openai_register" => Ok(WorkflowKind::OpenAIRegister),
            "openai_register_browser" => Ok(WorkflowKind::OpenAIRegisterBrowser),
            "grok_register" => Ok(WorkflowKind::GrokRegister),
            _ => Err(format!("不支持的工作流类型: {value}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        WorkflowDefinition, WorkflowEngine, select_registration_domain, validate_public_mail_domain,
    };

    #[test]
    fn registration_domain_skips_blank_workflow_override() {
        let resolved = select_registration_domain(Some("  "), Some("Mail.Example.COM."), None)
            .expect("system account domain should be selected");

        assert_eq!(resolved.value, "mail.example.com");
        assert_eq!(resolved.source, "account_domain");
    }

    #[test]
    fn registration_domain_falls_back_to_cloudflare_zone() {
        let resolved = select_registration_domain(None, None, Some("example.com"))
            .expect("Cloudflare zone domain should be selected");

        assert_eq!(resolved.value, "example.com");
        assert_eq!(resolved.source, "cloudflare_zone_domain");
    }

    #[test]
    fn public_mail_domain_validation_rejects_non_mail_targets() {
        assert!(validate_public_mail_domain("example.com").is_ok());
        assert!(validate_public_mail_domain("mail.example.com").is_ok());
        assert!(validate_public_mail_domain("phantom.local").is_err());
        assert!(validate_public_mail_domain("https://example.com").is_err());
        assert!(validate_public_mail_domain("inbox@example.com").is_err());
        assert!(validate_public_mail_domain("127.0.0.1").is_err());
    }

    #[test]
    fn grok_definition_validation_rejects_invalid_network_inputs() {
        let mut definition = WorkflowDefinition {
            id: "grok_register_default".to_string(),
            kind: super::WorkflowKind::GrokRegister,
            title: "Grok".to_string(),
            summary: String::new(),
            status: "ready".to_string(),
            builtin: true,
            parameters: super::WorkflowParameters::default(),
        };

        definition.parameters.account_domain = Some("https://example.com".to_string());
        assert!(WorkflowEngine::validate_parameters(&definition).is_err());

        definition.parameters.account_domain = Some("example.com".to_string());
        definition.parameters.proxy_url = Some("not a proxy".to_string());
        assert!(WorkflowEngine::validate_parameters(&definition).is_err());

        definition.parameters.proxy_url = None;
        definition.parameters.turnstile_solver_url = Some("file:///tmp/solver".to_string());
        assert!(WorkflowEngine::validate_parameters(&definition).is_err());
    }

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
