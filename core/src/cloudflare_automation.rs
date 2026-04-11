use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::process::Command;
use uuid::Uuid;

use crate::stream::{StreamHub, StreamPayload};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudflareAutomationRunPayload {
    pub mode: Option<String>,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CloudflareAutomationStatus {
    pub running: bool,
    pub current_step: Option<String>,
    pub last_started_at: Option<i64>,
    pub last_finished_at: Option<i64>,
    pub last_success: Option<bool>,
    pub last_mode: Option<String>,
    pub last_public_url: Option<String>,
    pub summary: Option<Value>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub error: Option<String>,
    pub logs: Vec<CloudflareAutomationLogEntry>,
    pub worker_url: Option<String>,
    pub email_address: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloudflareAutomationLogEntry {
    pub level: String,
    pub message: String,
}

#[derive(Clone)]
pub struct CloudflareAutomationManager {
    status: Arc<Mutex<CloudflareAutomationStatus>>,
    project_root: PathBuf,
    stream_hub: Arc<StreamHub>,
}

impl CloudflareAutomationManager {
    pub fn new(project_root: PathBuf, stream_hub: Arc<StreamHub>) -> Self {
        Self {
            status: Arc::new(Mutex::new(CloudflareAutomationStatus::default())),
            project_root,
            stream_hub,
        }
    }

    pub fn status(&self) -> CloudflareAutomationStatus {
        self.status
            .lock()
            .expect("Failed to lock automation status")
            .clone()
    }

    pub fn start(&self, payload: CloudflareAutomationRunPayload) -> Result<(), String> {
        {
            let mut status = self
                .status
                .lock()
                .expect("Failed to lock automation status");
            if status.running {
                return Err("Cloudflare automation is already running".to_string());
            }

            status.running = true;
            status.current_step = Some("准备执行 Cloudflare 自动化流程".to_string());
            status.last_started_at = Some(now_ts());
            status.last_finished_at = None;
            status.last_success = None;
            status.last_mode = payload.mode.clone();
            status.last_public_url = payload.public_url.clone();
            status.summary = None;
            status.stdout = None;
            status.stderr = None;
            status.error = None;
            status.logs = vec![CloudflareAutomationLogEntry {
                level: "step".to_string(),
                message: "准备执行 Cloudflare 自动化流程".to_string(),
            }];
            status.worker_url = None;
            status.email_address = None;
        }

        self.broadcast_log("info", "Cloudflare 自动化任务已启动");

        let manager = self.clone();
        tokio::spawn(async move {
            manager.run_task(payload).await;
        });

        Ok(())
    }

    async fn run_task(&self, payload: CloudflareAutomationRunPayload) {
        match self.execute_script(payload).await {
            Ok(result) => {
                let (logs, worker_url, email_address) = {
                    let mut status = self
                        .status
                        .lock()
                        .expect("Failed to lock automation status");
                    status.running = false;
                    status.current_step = Some("自动化流程完成".to_string());
                    status.last_finished_at = Some(now_ts());
                    status.last_success = Some(true);
                    status.summary = result.summary;
                    status.stdout = Some(result.stdout);
                    status.stderr = if result.stderr.trim().is_empty() {
                        None
                    } else {
                        Some(result.stderr)
                    };
                    status.error = None;
                    status.logs = build_logs(&status.stdout, &status.stderr, None);
                    status.worker_url = summary_string(status.summary.as_ref(), "worker_url");
                    status.email_address = summary_string(status.summary.as_ref(), "email_address");
                    (
                        status.logs.clone(),
                        status.worker_url.clone(),
                        status.email_address.clone(),
                    )
                };

                for log in logs {
                    self.broadcast_log(&map_log_level(&log.level), &log.message);
                }
                if let Some(worker_url) = worker_url {
                    self.broadcast_log("success", &format!("Worker URL 已生成: {worker_url}"));
                }
                if let Some(email_address) = email_address {
                    self.broadcast_log("success", &format!("最终邮箱地址已生成: {email_address}"));
                }
            }
            Err(error) => {
                let logs = {
                    let mut status = self
                        .status
                        .lock()
                        .expect("Failed to lock automation status");
                    status.running = false;
                    status.current_step = Some("自动化流程失败".to_string());
                    status.last_finished_at = Some(now_ts());
                    status.last_success = Some(false);
                    status.error = Some(error.message);
                    status.stdout = error.stdout;
                    status.stderr = error.stderr;
                    status.summary = read_summary_file(&self.project_root);
                    status.logs =
                        build_logs(&status.stdout, &status.stderr, status.error.as_deref());
                    status.worker_url = summary_string(status.summary.as_ref(), "worker_url");
                    status.email_address = summary_string(status.summary.as_ref(), "email_address");
                    status.logs.clone()
                };

                for log in logs {
                    self.broadcast_log(&map_log_level(&log.level), &log.message);
                }
            }
        }
    }

    async fn execute_script(
        &self,
        payload: CloudflareAutomationRunPayload,
    ) -> Result<ExecutionResult, ExecutionError> {
        self.set_step("调用 Cloudflare 自动化脚本");

        let script_path = self.project_root.join("setup-cloudflare-mail.ps1");
        if !script_path.exists() {
            return Err(ExecutionError::message(format!(
                "Automation script not found: {}",
                script_path.display()
            )));
        }

        let shell = shell_program();
        let mut command = Command::new(shell);

        if cfg!(windows) {
            command.arg("-ExecutionPolicy").arg("Bypass");
        }

        command.arg("-File").arg(&script_path);

        if let Some(mode) = payload
            .mode
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            command.arg("-Mode").arg(mode.trim());
        }

        if let Some(public_url) = payload
            .public_url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            command.arg("-PublicUrl").arg(public_url.trim());
        }

        command.current_dir(&self.project_root);

        let output = command.output().await.map_err(|error| {
            ExecutionError::message(format!("Failed to start automation command: {error}"))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(ExecutionError {
                message: format!(
                    "Automation command failed with exit code {}",
                    output.status.code().unwrap_or(-1)
                ),
                stdout: if stdout.trim().is_empty() {
                    None
                } else {
                    Some(stdout)
                },
                stderr: if stderr.trim().is_empty() {
                    None
                } else {
                    Some(stderr)
                },
            });
        }

        Ok(ExecutionResult {
            summary: read_summary_file(&self.project_root),
            stdout,
            stderr,
        })
    }

    fn set_step(&self, message: &str) {
        {
            let mut status = self
                .status
                .lock()
                .expect("Failed to lock automation status");
            status.current_step = Some(message.to_string());
            status.logs.push(CloudflareAutomationLogEntry {
                level: "step".to_string(),
                message: message.to_string(),
            });
        }
        self.broadcast_log("info", message);
    }

    fn broadcast_log(&self, level: &str, message: &str) {
        self.stream_hub.broadcast(StreamPayload {
            id: Uuid::new_v4().to_string(),
            event_type: "system_log".to_string(),
            data: serde_json::json!({
                "level": level,
                "msg": format!("Cloudflare 自动化 / {message}"),
            }),
        });
    }
}

fn shell_program() -> &'static str {
    if cfg!(windows) {
        "powershell.exe"
    } else {
        "pwsh"
    }
}

fn read_summary_file(project_root: &Path) -> Option<Value> {
    let summary_path = project_root
        .join(".automation")
        .join("cloudflare-mail-last-run.json");
    let raw = std::fs::read_to_string(summary_path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

fn summary_string(summary: Option<&Value>, key: &str) -> Option<String> {
    summary
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn build_logs(
    stdout: &Option<String>,
    stderr: &Option<String>,
    error_message: Option<&str>,
) -> Vec<CloudflareAutomationLogEntry> {
    let mut logs = Vec::new();

    let push_lines =
        |logs: &mut Vec<CloudflareAutomationLogEntry>, raw: &str, default_level: &str| {
            for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
                let (level, message) = if let Some(message) = line.strip_prefix("[STEP] ") {
                    ("step", message)
                } else if let Some(message) = line.strip_prefix("[ OK ] ") {
                    ("success", message)
                } else if let Some(message) = line.strip_prefix("[WARN] ") {
                    ("warn", message)
                } else if let Some(message) = line.strip_prefix("[INFO] ") {
                    ("info", message)
                } else {
                    (default_level, line)
                };

                logs.push(CloudflareAutomationLogEntry {
                    level: level.to_string(),
                    message: message.to_string(),
                });
            }
        };

    if let Some(stdout) = stdout {
        push_lines(&mut logs, stdout, "info");
    }
    if let Some(stderr) = stderr {
        push_lines(&mut logs, stderr, "warn");
    }
    if let Some(error_message) = error_message.filter(|value| !value.trim().is_empty()) {
        logs.push(CloudflareAutomationLogEntry {
            level: "error".to_string(),
            message: error_message.to_string(),
        });
    }

    logs
}

fn map_log_level(level: &str) -> String {
    match level {
        "success" => "success".to_string(),
        "warn" => "warn".to_string(),
        "error" => "warn".to_string(),
        _ => "info".to_string(),
    }
}

struct ExecutionResult {
    summary: Option<Value>,
    stdout: String,
    stderr: String,
}

struct ExecutionError {
    message: String,
    stdout: Option<String>,
    stderr: Option<String>,
}

impl ExecutionError {
    fn message(message: String) -> Self {
        Self {
            message,
            stdout: None,
            stderr: None,
        }
    }
}
