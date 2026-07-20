use chrono::Utc;
use sqlx::{Pool, Row, Sqlite, sqlite::SqlitePoolOptions};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct AdminCredentials {
    pub username: String,
    pub password_hash: String,
}

/**
 * 幻影中台 - 数据湖基座 (Data Lake)
 * 职责：管理所有热数据 (SQLite 映射) 与归档数据 (CSV.GZ)
 */

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct EmailRecord {
    pub id: String,
    pub created_at: i64,
    pub from_addr: String,
    pub to_addr: String,
    pub subject: Option<String>,
    pub extracted_code: Option<String>,
    pub extracted_link: Option<String>,
    pub extracted_text: Option<String>,
    pub is_archived: bool,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct EmailDetailRecord {
    pub id: String,
    pub created_at: i64,
    pub from_addr: String,
    pub to_addr: String,
    pub subject: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub extracted_code: Option<String>,
    pub extracted_link: Option<String>,
    pub extracted_text: Option<String>,
    pub is_archived: bool,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct WorkflowRunRecord {
    pub id: String,
    pub workflow_id: String,
    pub workflow_title: String,
    pub status: String,
    pub message: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct WorkflowRunPage {
    pub items: Vec<WorkflowRunRecord>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct WorkflowDefinitionRecord {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub parameters_json: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct WorkflowStepRecord {
    pub id: String,
    pub run_id: String,
    pub step_index: i64,
    pub level: String,
    pub message: String,
    pub created_at: i64,
    pub workflow_id: Option<String>,
    pub workflow_title: Option<String>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct GeneratedAccountRecord {
    pub id: String,
    pub run_id: String,
    pub address: String,
    pub password: String,
    pub status: String,
    pub created_at: i64,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub session_token: Option<String>,
    pub id_token: Option<String>,
    pub device_id: Option<String>,
    pub workspace_id: Option<String>,
    pub chatgpt_account_id: Option<String>,
    pub chatgpt_user_id: Option<String>,
    pub organization_id: Option<String>,
    pub plan_type: Option<String>,
    pub expires_in: Option<i64>,
    pub token_version: Option<i64>,
    pub oauth_credentials_json: Option<String>,
    pub upload_status: Option<String>,
    pub account_type: Option<String>,
    pub proxy_url: Option<String>,
    pub pool_tag: Option<String>,
    pub last_used_at: Option<i64>,
    pub rate_limit_reset_at: Option<i64>,
    pub consecutive_failures: Option<i64>,
    pub request_count_24h: Option<i64>,
    pub last_failure_reason: Option<String>,
    pub proxy_rtt: Option<i64>,
    pub proxy_ip_type: Option<String>,
    pub proxy_status: Option<String>,
    pub proxy_last_checked_at: Option<i64>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct DashboardStats {
    pub total_emails: i64,
    pub active_emails: i64,
    pub archived_emails: i64,
    pub code_emails: i64,
    pub recent_emails_24h: i64,
    pub active_webhooks: i64,
    pub workflow_runs_24h: i64,
    pub successful_runs_24h: i64,
    pub total_accounts: i64,
    pub today_accounts_24h: i64,
    pub gateway_requests_24h: i64,
    pub active_pool_accounts: i64,
    pub cooling_accounts: i64,
    pub latest_email_at: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct EmailPage {
    pub items: Vec<EmailRecord>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(sqlx::FromRow)]
pub struct WebhookOutboxRecord {
    pub id: String,
    pub webhook_url: String,
    pub payload: String,
    pub attempts: i64,
}

pub struct DataLake {
    pub pool: Pool<Sqlite>,
}

impl DataLake {
    const GENERATED_ACCOUNT_COLUMNS: &'static str =
        "id, run_id, address, password, status, created_at,
                    access_token, refresh_token, session_token, id_token,
                    device_id, workspace_id, chatgpt_account_id, chatgpt_user_id,
                    organization_id, plan_type, expires_in, token_version, oauth_credentials_json,
                    upload_status, account_type, proxy_url, pool_tag, last_used_at, rate_limit_reset_at,
                    consecutive_failures, request_count_24h, last_failure_reason,
                    proxy_rtt, proxy_ip_type, proxy_status, proxy_last_checked_at";

    /// 初始化数据湖连接并确保表结构存在
    pub async fn new(database_url: &str) -> Arc<Self> {
        // 使用高性能连接池
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .expect("无法连接到 SQLite 数据湖");

        Self::configure_sqlite(&pool).await;
        Self::prepare_legacy_data_for_migrations(&pool).await;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("数据库迁移失败");
        Self::ensure_legacy_columns(&pool).await;

        Arc::new(Self { pool })
    }

    async fn configure_sqlite(pool: &Pool<Sqlite>) {
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(pool)
            .await
            .expect("SQLite WAL 模式配置失败");
        sqlx::query("PRAGMA synchronous = NORMAL")
            .execute(pool)
            .await
            .expect("SQLite 同步模式配置失败");
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(pool)
            .await
            .expect("SQLite busy timeout 配置失败");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(pool)
            .await
            .expect("SQLite 外键配置失败");
    }

    async fn prepare_legacy_data_for_migrations(pool: &Pool<Sqlite>) {
        let migration_table_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
        )
        .fetch_one(pool)
        .await
        .expect("检查迁移记录表失败");
        if migration_table_exists > 0 {
            let migration_applied: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 202607190001 AND success = 1",
            )
            .fetch_one(pool)
            .await
            .expect("检查性能迁移状态失败");
            if migration_applied > 0 {
                return;
            }
        }

        let steps_table_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'workflow_run_steps'",
        )
        .fetch_one(pool)
        .await
        .expect("检查旧工作流步骤表失败");
        if steps_table_exists == 0 {
            return;
        }

        let result = sqlx::query(
            "DELETE FROM workflow_run_steps
             WHERE rowid NOT IN (
                 SELECT MIN(rowid)
                 FROM workflow_run_steps
                 GROUP BY run_id, step_index
             )",
        )
        .execute(pool)
        .await
        .expect("清理旧工作流重复步骤失败");
        if result.rows_affected() > 0 {
            eprintln!(
                "数据库升级：已清理 {} 条重复工作流步骤",
                result.rows_affected()
            );
        }
    }

    async fn ensure_legacy_columns(pool: &Pool<Sqlite>) {
        Self::add_column_if_missing(pool, "emails", "extracted_link", "TEXT").await;
        Self::add_column_if_missing(pool, "emails", "extracted_text", "TEXT").await;
        Self::add_column_if_missing(
            pool,
            "workflow_definitions",
            "kind",
            "TEXT NOT NULL DEFAULT 'account_generate'",
        )
        .await;
        Self::add_column_if_missing(pool, "generated_accounts", "access_token", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "refresh_token", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "session_token", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "id_token", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "device_id", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "workspace_id", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "chatgpt_account_id", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "chatgpt_user_id", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "organization_id", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "plan_type", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "expires_in", "INTEGER").await;
        Self::add_column_if_missing(pool, "generated_accounts", "token_version", "INTEGER").await;
        Self::add_column_if_missing(pool, "generated_accounts", "oauth_credentials_json", "TEXT")
            .await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "upload_status",
            "TEXT DEFAULT 'pending'",
        )
        .await;
        Self::add_column_if_missing(pool, "generated_accounts", "account_type", "TEXT").await;
        Self::add_column_if_missing(pool, "generated_accounts", "proxy_url", "TEXT").await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "pool_tag",
            "TEXT DEFAULT 'default'",
        )
        .await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "last_used_at",
            "INTEGER DEFAULT 0",
        )
        .await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "rate_limit_reset_at",
            "INTEGER DEFAULT 0",
        )
        .await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "consecutive_failures",
            "INTEGER DEFAULT 0",
        )
        .await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "request_count_24h",
            "INTEGER DEFAULT 0",
        )
        .await;
        Self::add_column_if_missing(pool, "generated_accounts", "last_failure_reason", "TEXT")
            .await;
        Self::add_column_if_missing(pool, "generated_accounts", "proxy_rtt", "INTEGER DEFAULT 0")
            .await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "proxy_ip_type",
            "TEXT DEFAULT 'unknown'",
        )
        .await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "proxy_status",
            "TEXT DEFAULT 'active'",
        )
        .await;
        Self::add_column_if_missing(
            pool,
            "generated_accounts",
            "proxy_last_checked_at",
            "INTEGER DEFAULT 0",
        )
        .await;
    }

    async fn add_column_if_missing(
        pool: &Pool<Sqlite>,
        table: &str,
        column: &str,
        definition: &str,
    ) {
        if Self::table_has_column(pool, table, column).await {
            return;
        }

        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
        sqlx::query(&sql)
            .execute(pool)
            .await
            .unwrap_or_else(|error| panic!("数据库列迁移失败: {table}.{column}: {error}"));
    }

    async fn table_has_column(pool: &Pool<Sqlite>, table: &str, column: &str) -> bool {
        let sql = format!("PRAGMA table_info({table})");
        sqlx::query(&sql)
            .fetch_all(pool)
            .await
            .map(|rows| {
                rows.iter()
                    .any(|row| row.get::<String, _>("name") == column)
            })
            .unwrap_or(false)
    }

    /// Insert an email and its webhook delivery jobs in one transaction.
    pub async fn record_email(
        &self,
        id: &str,
        from: &str,
        to: &str,
        subject: &str,
        text: &str,
        html: &str,
        extracted_code: Option<&str>,
        extracted_link: Option<&str>,
        extracted_text: Option<&str>,
        webhook_payload: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().timestamp();
        let to_lower = to.trim().to_lowercase();
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO emails (id, created_at, from_addr, to_addr, subject, body_text, body_html, extracted_code, extracted_link, extracted_text)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(now)
        .bind(from)
        .bind(&to_lower)
        .bind(subject)
        .bind(text)
        .bind(html)
        .bind(extracted_code)
        .bind(extracted_link)
        .bind(extracted_text)
        .execute(&mut *transaction)
        .await?;

        if let Some(payload) = webhook_payload {
            sqlx::query(
                "INSERT INTO webhook_outbox (
                    id, webhook_url, payload, status, attempts,
                    next_attempt_at, created_at, updated_at
                 )
                 SELECT lower(hex(randomblob(16))), url, ?, 'pending', 0, ?, ?, ?
                 FROM webhooks
                 WHERE is_active = 1
                   AND (event_filter = '*' OR event_filter = 'EMAIL_INGEST_READY')",
            )
            .bind(payload)
            .bind(now)
            .bind(now)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
    /// 获取最新的邮件列表（支持限制条数）
    pub async fn get_emails(
        &self,
        limit: i64,
        query: Option<&str>,
        archived: Option<bool>,
    ) -> Result<Vec<EmailRecord>, sqlx::Error> {
        let normalized_limit = limit.clamp(1, 500);
        let archived_filter = archived;

        if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
            let like = format!("%{}%", query.trim().to_lowercase());
            let sql = if archived_filter.is_some() {
                "SELECT id, created_at, from_addr, to_addr, subject, extracted_code, extracted_link, extracted_text, is_archived
                 FROM emails
                 WHERE (
                    lower(from_addr) LIKE ?
                    OR lower(to_addr) LIKE ?
                    OR lower(COALESCE(subject, '')) LIKE ?
                    OR lower(COALESCE(body_text, '')) LIKE ?
                    OR lower(COALESCE(body_html, '')) LIKE ?
                    OR lower(COALESCE(extracted_code, '')) LIKE ?
                    OR lower(COALESCE(extracted_link, '')) LIKE ?
                    OR lower(COALESCE(extracted_text, '')) LIKE ?
                 ) AND is_archived = ?
                 ORDER BY created_at DESC
                 LIMIT ?"
            } else {
                "SELECT id, created_at, from_addr, to_addr, subject, extracted_code, extracted_link, extracted_text, is_archived
                 FROM emails
                 WHERE lower(from_addr) LIKE ?
                    OR lower(to_addr) LIKE ?
                    OR lower(COALESCE(subject, '')) LIKE ?
                    OR lower(COALESCE(body_text, '')) LIKE ?
                    OR lower(COALESCE(body_html, '')) LIKE ?
                    OR lower(COALESCE(extracted_code, '')) LIKE ?
                    OR lower(COALESCE(extracted_link, '')) LIKE ?
                    OR lower(COALESCE(extracted_text, '')) LIKE ?
                 ORDER BY created_at DESC
                 LIMIT ?"
            };

            let mut query_builder = sqlx::query_as::<_, EmailRecord>(sql)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like);

            if let Some(archived_value) = archived_filter {
                query_builder = query_builder.bind(archived_value);
            }

            let records = query_builder
                .bind(normalized_limit)
                .fetch_all(&self.pool)
                .await?;
            Ok(records)
        } else {
            let sql = if archived_filter.is_some() {
                "SELECT id, created_at, from_addr, to_addr, subject, extracted_code, extracted_link, extracted_text, is_archived
                 FROM emails
                 WHERE is_archived = ?
                 ORDER BY created_at DESC
                 LIMIT ?"
            } else {
                "SELECT id, created_at, from_addr, to_addr, subject, extracted_code, extracted_link, extracted_text, is_archived
                 FROM emails
                 ORDER BY created_at DESC
                 LIMIT ?"
            };

            let mut query_builder = sqlx::query_as::<_, EmailRecord>(sql);
            if let Some(archived_value) = archived_filter {
                query_builder = query_builder.bind(archived_value);
            }

            let records = query_builder
                .bind(normalized_limit)
                .fetch_all(&self.pool)
                .await?;
            Ok(records)
        }
    }

    /// 分页获取邮件列表与总数
    pub async fn get_emails_page(
        &self,
        page: i64,
        page_size: i64,
        query: Option<&str>,
        archived: Option<bool>,
    ) -> Result<EmailPage, sqlx::Error> {
        let normalized_page = page.max(1);
        let normalized_page_size = page_size.clamp(1, 200);
        let offset = (normalized_page - 1) * normalized_page_size;
        let archived_filter = archived;

        if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
            let like = format!("%{}%", query.trim().to_lowercase());
            let total_sql = if archived_filter.is_some() {
                "SELECT COUNT(*)
                 FROM emails
                 WHERE (
                    lower(from_addr) LIKE ?
                    OR lower(to_addr) LIKE ?
                    OR lower(COALESCE(subject, '')) LIKE ?
                    OR lower(COALESCE(body_text, '')) LIKE ?
                    OR lower(COALESCE(body_html, '')) LIKE ?
                    OR lower(COALESCE(extracted_code, '')) LIKE ?
                    OR lower(COALESCE(extracted_link, '')) LIKE ?
                    OR lower(COALESCE(extracted_text, '')) LIKE ?
                 ) AND is_archived = ?"
            } else {
                "SELECT COUNT(*)
                 FROM emails
                 WHERE lower(from_addr) LIKE ?
                    OR lower(to_addr) LIKE ?
                    OR lower(COALESCE(subject, '')) LIKE ?
                    OR lower(COALESCE(body_text, '')) LIKE ?
                    OR lower(COALESCE(body_html, '')) LIKE ?
                    OR lower(COALESCE(extracted_code, '')) LIKE ?
                    OR lower(COALESCE(extracted_link, '')) LIKE ?
                    OR lower(COALESCE(extracted_text, '')) LIKE ?"
            };

            let mut total_query = sqlx::query_scalar::<_, i64>(total_sql)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like);

            if let Some(archived_value) = archived_filter {
                total_query = total_query.bind(archived_value);
            }

            let total = total_query.fetch_one(&self.pool).await?;

            let items_sql = if archived_filter.is_some() {
                "SELECT id, created_at, from_addr, to_addr, subject, extracted_code, extracted_link, extracted_text, is_archived
                 FROM emails
                 WHERE (
                    lower(from_addr) LIKE ?
                    OR lower(to_addr) LIKE ?
                    OR lower(COALESCE(subject, '')) LIKE ?
                    OR lower(COALESCE(body_text, '')) LIKE ?
                    OR lower(COALESCE(body_html, '')) LIKE ?
                    OR lower(COALESCE(extracted_code, '')) LIKE ?
                    OR lower(COALESCE(extracted_link, '')) LIKE ?
                    OR lower(COALESCE(extracted_text, '')) LIKE ?
                 ) AND is_archived = ?
                 ORDER BY created_at DESC
                 LIMIT ?
                 OFFSET ?"
            } else {
                "SELECT id, created_at, from_addr, to_addr, subject, extracted_code, extracted_link, extracted_text, is_archived
                 FROM emails
                 WHERE lower(from_addr) LIKE ?
                    OR lower(to_addr) LIKE ?
                    OR lower(COALESCE(subject, '')) LIKE ?
                    OR lower(COALESCE(body_text, '')) LIKE ?
                    OR lower(COALESCE(body_html, '')) LIKE ?
                    OR lower(COALESCE(extracted_code, '')) LIKE ?
                    OR lower(COALESCE(extracted_link, '')) LIKE ?
                    OR lower(COALESCE(extracted_text, '')) LIKE ?
                 ORDER BY created_at DESC
                 LIMIT ?
                 OFFSET ?"
            };

            let mut items_query = sqlx::query_as::<_, EmailRecord>(items_sql)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(&like);

            if let Some(archived_value) = archived_filter {
                items_query = items_query.bind(archived_value);
            }

            let items = items_query
                .bind(normalized_page_size)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;

            Ok(EmailPage {
                items,
                total,
                page: normalized_page,
                page_size: normalized_page_size,
            })
        } else {
            let total_sql = if archived_filter.is_some() {
                "SELECT COUNT(*) FROM emails WHERE is_archived = ?"
            } else {
                "SELECT COUNT(*) FROM emails"
            };

            let mut total_query = sqlx::query_scalar::<_, i64>(total_sql);
            if let Some(archived_value) = archived_filter {
                total_query = total_query.bind(archived_value);
            }
            let total = total_query.fetch_one(&self.pool).await?;

            let items_sql = if archived_filter.is_some() {
                "SELECT id, created_at, from_addr, to_addr, subject, extracted_code, extracted_link, extracted_text, is_archived
                 FROM emails
                 WHERE is_archived = ?
                 ORDER BY created_at DESC
                 LIMIT ?
                 OFFSET ?"
            } else {
                "SELECT id, created_at, from_addr, to_addr, subject, extracted_code, extracted_link, extracted_text, is_archived
                 FROM emails
                 ORDER BY created_at DESC
                 LIMIT ?
                 OFFSET ?"
            };

            let mut items_query = sqlx::query_as::<_, EmailRecord>(items_sql);
            if let Some(archived_value) = archived_filter {
                items_query = items_query.bind(archived_value);
            }

            let items = items_query
                .bind(normalized_page_size)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;

            Ok(EmailPage {
                items,
                total,
                page: normalized_page,
                page_size: normalized_page_size,
            })
        }
    }

    /// 获取邮件详情
    pub async fn get_email_detail(
        &self,
        id: &str,
    ) -> Result<Option<EmailDetailRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, EmailDetailRecord>(
            "SELECT id, created_at, from_addr, to_addr, subject, body_text, body_html, extracted_code, extracted_link, extracted_text, is_archived
             FROM emails
             WHERE id = ?
             LIMIT 1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    /// 归档或取消归档邮件
    pub async fn set_email_archived(&self, id: &str, archived: bool) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("UPDATE emails SET is_archived = ? WHERE id = ?")
            .bind(archived)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// 删除邮件
    pub async fn delete_email(&self, id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM emails WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// 批量归档或取消归档邮件
    pub async fn set_emails_archived(
        &self,
        ids: &[String],
        archived: bool,
    ) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }

        let placeholders = vec!["?"; ids.len()].join(", ");
        let sql = format!("UPDATE emails SET is_archived = ? WHERE id IN ({placeholders})");
        let mut query = sqlx::query(&sql).bind(archived);
        for id in ids {
            query = query.bind(id);
        }

        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    /// 批量删除邮件
    pub async fn delete_emails(&self, ids: &[String]) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }

        let placeholders = vec!["?"; ids.len()].join(", ");
        let sql = format!("DELETE FROM emails WHERE id IN ({placeholders})");
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    /// 获取仪表盘真实统计数据
    pub async fn get_dashboard_stats(&self) -> Result<DashboardStats, sqlx::Error> {
        let now = Utc::now().timestamp();
        let threshold_24h = now - 24 * 3600;

        let stats = sqlx::query_as::<_, DashboardStats>(
            "SELECT
                (SELECT COUNT(*) FROM emails) AS total_emails,
                (SELECT COUNT(*) FROM emails WHERE is_archived = 0) AS active_emails,
                (SELECT COUNT(*) FROM emails WHERE is_archived = 1) AS archived_emails,
                (SELECT COUNT(*) FROM emails WHERE extracted_code IS NOT NULL AND extracted_code != '') AS code_emails,
                (SELECT COUNT(*) FROM emails WHERE created_at >= ?) AS recent_emails_24h,
                (SELECT COUNT(*) FROM webhooks WHERE is_active = 1) AS active_webhooks,
                (SELECT COUNT(*) FROM workflow_runs WHERE started_at >= ?) AS workflow_runs_24h,
                (SELECT COUNT(*) FROM workflow_runs WHERE started_at >= ? AND status = 'success') AS successful_runs_24h,
                (SELECT COUNT(*) FROM generated_accounts) AS total_accounts,
                (SELECT COUNT(*) FROM generated_accounts WHERE created_at >= ?) AS today_accounts_24h,
                (SELECT COALESCE(SUM(request_count_24h), 0) FROM generated_accounts) AS gateway_requests_24h,
                (SELECT COUNT(*) FROM generated_accounts WHERE (status LIKE '%registered%' OR lower(status) LIKE '%success%') AND access_token IS NOT NULL AND access_token != '' AND (rate_limit_reset_at IS NULL OR rate_limit_reset_at <= ?)) AS active_pool_accounts,
                (SELECT COUNT(*) FROM generated_accounts WHERE rate_limit_reset_at > ?) AS cooling_accounts,
                (SELECT MAX(created_at) FROM emails) AS latest_email_at"
        )
        .bind(threshold_24h)
        .bind(threshold_24h)
        .bind(threshold_24h)
        .bind(threshold_24h)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(stats)
    }

    /// 清理历史数据 (保留最近 N 天)
    pub async fn cleanup_emails(&self, days_to_keep: i64) -> Result<u64, sqlx::Error> {
        let threshold = Utc::now().timestamp() - (days_to_keep * 24 * 3600);
        let result = sqlx::query("DELETE FROM emails WHERE created_at < ?")
            .bind(threshold)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// 获取所有活跃的 Webhook
    pub async fn get_active_webhooks(&self) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows = sqlx::query("SELECT url, event_filter FROM webhooks WHERE is_active = 1")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|r: sqlx::sqlite::SqliteRow| {
                use sqlx::Row;
                (r.get("url"), r.get("event_filter"))
            })
            .collect())
    }

    pub async fn recover_webhook_outbox(&self) -> Result<u64, sqlx::Error> {
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            "UPDATE webhook_outbox SET status = 'pending', updated_at = ?
             WHERE status = 'delivering' AND updated_at <= ?",
        )
        .bind(now)
        .bind(now - 60)
        .execute(&self.pool)
        .await?;
        sqlx::query("DELETE FROM webhook_outbox WHERE status = 'failed' AND updated_at < ?")
            .bind(now - 30 * 24 * 60 * 60)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn lease_webhook_delivery(&self) -> Result<Option<WebhookOutboxRecord>, sqlx::Error> {
        let now = Utc::now().timestamp();
        sqlx::query_as::<_, WebhookOutboxRecord>(
            "UPDATE webhook_outbox SET status = 'delivering', updated_at = ?
             WHERE id = (
                 SELECT id FROM webhook_outbox
                 WHERE status = 'pending' AND next_attempt_at <= ?
                 ORDER BY next_attempt_at, created_at LIMIT 1
             )
             RETURNING id, webhook_url, payload, attempts",
        )
        .bind(now)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn complete_webhook_delivery(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM webhook_outbox WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn fail_webhook_delivery(
        &self,
        id: &str,
        previous_attempts: i64,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        let attempts = previous_attempts + 1;
        let status = if attempts >= 8 { "failed" } else { "pending" };
        let delay_seconds = (1_i64 << attempts.min(8) as u32).min(300);
        let shortened_error: String = error.chars().take(1_000).collect();
        sqlx::query(
            "UPDATE webhook_outbox
             SET status = ?, attempts = ?, next_attempt_at = ?, last_error = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(attempts)
        .bind(Utc::now().timestamp() + delay_seconds)
        .bind(shortened_error)
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    pub async fn recover_interrupted_workflow_runs(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE workflow_runs
             SET status = 'interrupted',
                 message = '服务重启前任务未完成，请重新执行',
                 finished_at = ?
             WHERE status = 'running'",
        )
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
    /// 创建工作流执行记录
    pub async fn create_workflow_run(
        &self,
        id: &str,
        workflow_id: &str,
        workflow_title: &str,
        status: &str,
        message: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO workflow_runs (id, workflow_id, workflow_title, status, message, started_at)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(id)
        .bind(workflow_id)
        .bind(workflow_title)
        .bind(status)
        .bind(message)
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 更新工作流执行状态
    pub async fn finish_workflow_run(
        &self,
        id: &str,
        status: &str,
        message: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE workflow_runs
             SET status = ?, message = ?, finished_at = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(message)
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 获取单个工作流运行的状态
    pub async fn get_workflow_run_status(&self, id: &str) -> Result<String, sqlx::Error> {
        let status: (String,) = sqlx::query_as("SELECT status FROM workflow_runs WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(status.0)
    }

    pub async fn stop_workflow_run(&self, id: &str) -> Result<u64, sqlx::Error> {
        let now = Utc::now().timestamp();
        let result = sqlx::query("UPDATE workflow_runs SET status = 'cancelled', message = '用户手动终止', finished_at = ? WHERE id = ? AND status = 'running'")
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// 分页获取工作流执行记录
    pub async fn get_workflow_runs_page(
        &self,
        page: i64,
        page_size: i64,
        status: Option<&str>,
        workflow_id: Option<&str>,
        workflow_exact: bool,
    ) -> Result<WorkflowRunPage, sqlx::Error> {
        let normalized_page = page.max(1);
        let normalized_page_size = page_size.clamp(1, 100);
        let offset = (normalized_page - 1) * normalized_page_size;
        let status_filter = status
            .filter(|value| !value.trim().is_empty())
            .map(str::trim);
        let workflow_filter = workflow_id
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_lowercase());

        let total_sql = match (status_filter.is_some(), workflow_filter.is_some()) {
            (true, true) => {
                "SELECT COUNT(*)
                 FROM workflow_runs
                 WHERE status = ? AND lower(workflow_id) = ?"
            }
            (true, false) => {
                "SELECT COUNT(*)
                 FROM workflow_runs
                 WHERE status = ?"
            }
            (false, true) => {
                "SELECT COUNT(*)
                 FROM workflow_runs
                 WHERE lower(workflow_id) = ?"
            }
            (false, false) => "SELECT COUNT(*) FROM workflow_runs",
        };

        let total_sql = if workflow_filter.is_some() && !workflow_exact {
            total_sql.replace("lower(workflow_id) = ?", "lower(workflow_id) LIKE ?")
        } else {
            total_sql.to_string()
        };

        let mut total_query = sqlx::query_scalar::<_, i64>(&total_sql);
        if let Some(status_value) = status_filter {
            total_query = total_query.bind(status_value);
        }
        if let Some(workflow_value) = workflow_filter.as_ref() {
            let filter_value = if workflow_exact {
                workflow_value.clone()
            } else {
                format!("%{workflow_value}%")
            };
            total_query = total_query.bind(filter_value);
        }
        let total = total_query.fetch_one(&self.pool).await?;

        let items_sql = match (status_filter.is_some(), workflow_filter.is_some()) {
            (true, true) => {
                "SELECT id, workflow_id, workflow_title, status, message, started_at, finished_at
                 FROM workflow_runs
                 WHERE status = ? AND lower(workflow_id) = ?
                 ORDER BY started_at DESC
                 LIMIT ?
                 OFFSET ?"
            }
            (true, false) => {
                "SELECT id, workflow_id, workflow_title, status, message, started_at, finished_at
                 FROM workflow_runs
                 WHERE status = ?
                 ORDER BY started_at DESC
                 LIMIT ?
                 OFFSET ?"
            }
            (false, true) => {
                "SELECT id, workflow_id, workflow_title, status, message, started_at, finished_at
                 FROM workflow_runs
                 WHERE lower(workflow_id) = ?
                 ORDER BY started_at DESC
                 LIMIT ?
                 OFFSET ?"
            }
            (false, false) => {
                "SELECT id, workflow_id, workflow_title, status, message, started_at, finished_at
                 FROM workflow_runs
                 ORDER BY started_at DESC
                 LIMIT ?
                 OFFSET ?"
            }
        };

        let items_sql = if workflow_filter.is_some() && !workflow_exact {
            items_sql.replace("lower(workflow_id) = ?", "lower(workflow_id) LIKE ?")
        } else {
            items_sql.to_string()
        };

        let mut items_query = sqlx::query_as::<_, WorkflowRunRecord>(&items_sql);
        if let Some(status_value) = status_filter {
            items_query = items_query.bind(status_value);
        }
        if let Some(workflow_value) = workflow_filter.as_ref() {
            let filter_value = if workflow_exact {
                workflow_value.clone()
            } else {
                format!("%{workflow_value}%")
            };
            items_query = items_query.bind(filter_value);
        }
        let items = items_query
            .bind(normalized_page_size)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        Ok(WorkflowRunPage {
            items,
            total,
            page: normalized_page,
            page_size: normalized_page_size,
        })
    }

    /// 列出全部工作流定义
    pub async fn list_workflow_definitions(
        &self,
    ) -> Result<Vec<WorkflowDefinitionRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, WorkflowDefinitionRecord>(
            "SELECT id, kind, title, summary, status, parameters_json, created_at, updated_at
             FROM workflow_definitions
             ORDER BY updated_at DESC, created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// 写入或更新工作流定义
    pub async fn upsert_workflow_definition(
        &self,
        id: &str,
        kind: &str,
        title: &str,
        summary: &str,
        status: &str,
        parameters_json: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO workflow_definitions (id, kind, title, summary, status, parameters_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               kind = excluded.kind,
               title = excluded.title,
               summary = excluded.summary,
               status = excluded.status,
               parameters_json = excluded.parameters_json,
               updated_at = excluded.updated_at"
        )
        .bind(id)
        .bind(kind)
        .bind(title)
        .bind(summary)
        .bind(status)
        .bind(parameters_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 删除工作流定义
    pub async fn delete_workflow_definition(&self, id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM workflow_definitions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// 记录工作流执行步骤
    pub async fn add_workflow_step(
        &self,
        run_id: &str,
        step_index: i64,
        level: &str,
        message: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO workflow_run_steps (id, run_id, step_index, level, message, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(run_id)
        .bind(step_index)
        .bind(level)
        .bind(message)
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 记录批量生成得到的账号产物
    pub async fn create_generated_account(
        &self,
        run_id: &str,
        address: &str,
        password: &str,
        status: &str,
        account_type: Option<&str>,
        proxy_url: Option<&str>,
    ) -> Result<String, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let res = sqlx::query(
            "INSERT INTO generated_accounts (id, run_id, address, password, status, created_at, upload_status, account_type, proxy_url)
             VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?)"
        )
        .bind(&id)
        .bind(run_id)
        .bind(address)
        .bind(password)
        .bind(status)
        .bind(Utc::now().timestamp())
        .bind(account_type)
        .bind(proxy_url)
        .execute(&self.pool)
        .await;

        if let Err(ref e) = res {
            eprintln!("🔴 [数据库错误] 无法插入生成的账号: {e:?}");
        }
        res?;

        Ok(id)
    }

    /// 更新账号的 Token 信息（注册第二阶段产物）
    pub async fn update_account_tokens(
        &self,
        account_id: &str,
        access_token: Option<&str>,
        refresh_token: Option<&str>,
        session_token: Option<&str>,
        device_id: Option<&str>,
        workspace_id: Option<&str>,
        id_token: Option<&str>,
        chatgpt_account_id: Option<&str>,
        chatgpt_user_id: Option<&str>,
        organization_id: Option<&str>,
        plan_type: Option<&str>,
        expires_in: Option<i64>,
        token_version: Option<i64>,
        oauth_credentials_json: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE generated_accounts
             SET access_token = ?, refresh_token = ?, session_token = ?,
                 device_id = ?, workspace_id = ?, id_token = ?,
                 chatgpt_account_id = ?, chatgpt_user_id = ?, organization_id = ?,
                 plan_type = ?, expires_in = ?, token_version = ?, oauth_credentials_json = ?
             WHERE id = ?",
        )
        .bind(access_token)
        .bind(refresh_token)
        .bind(session_token)
        .bind(device_id)
        .bind(workspace_id)
        .bind(id_token)
        .bind(chatgpt_account_id)
        .bind(chatgpt_user_id)
        .bind(organization_id)
        .bind(plan_type)
        .bind(expires_in)
        .bind(token_version)
        .bind(oauth_credentials_json)
        .bind(account_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 更新账号的上传分发状态
    pub async fn update_account_upload_status(
        &self,
        account_id: &str,
        upload_status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE generated_accounts SET upload_status = ? WHERE id = ?")
            .bind(upload_status)
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// 更新账号的代理质量数据
    pub async fn update_proxy_quality(
        &self,
        account_id: &str,
        rtt: i64,
        ip_type: &str,
        status: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE generated_accounts 
             SET proxy_rtt = ?, proxy_ip_type = ?, proxy_status = ?, proxy_last_checked_at = ? 
             WHERE id = ?",
        )
        .bind(rtt)
        .bind(ip_type)
        .bind(status)
        .bind(Utc::now().timestamp())
        .bind(account_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 获取全部绑定了代理的且非熔断非封禁账号记录用于心跳检测
    pub async fn list_all_accounts_with_proxies(
        &self,
    ) -> Result<Vec<GeneratedAccountRecord>, sqlx::Error> {
        let sql = format!(
            "SELECT {}
             FROM generated_accounts
             WHERE proxy_url IS NOT NULL AND proxy_url != ''
               AND lower(status) NOT LIKE '%zombie%'
               AND lower(status) NOT LIKE '%banned%'
               AND lower(status) NOT LIKE '%expired%'",
            Self::GENERATED_ACCOUNT_COLUMNS
        );
        let records = sqlx::query_as::<_, GeneratedAccountRecord>(&sql)
            .fetch_all(&self.pool)
            .await?;
        Ok(records)
    }

    /// 获取某次运行生成的账号产物
    pub async fn list_generated_accounts(
        &self,
        run_id: &str,
        limit: i64,
    ) -> Result<Vec<GeneratedAccountRecord>, sqlx::Error> {
        let sql = if run_id == "all" {
            format!(
                "SELECT {}
             FROM generated_accounts
             ORDER BY created_at DESC
             LIMIT ?",
                Self::GENERATED_ACCOUNT_COLUMNS
            )
        } else {
            format!(
                "SELECT {}
             FROM generated_accounts
             WHERE run_id = ?
             ORDER BY created_at DESC
             LIMIT ?",
                Self::GENERATED_ACCOUNT_COLUMNS
            )
        };

        let mut query = sqlx::query_as::<_, GeneratedAccountRecord>(&sql);
        if run_id != "all" {
            query = query.bind(run_id);
        }
        let records = query
            .bind(limit.clamp(1, 10000))
            .fetch_all(&self.pool)
            .await?;

        Ok(records)
    }

    /// 获取全局账号列表（支持分页与搜索）
    pub async fn list_all_accounts(
        &self,
        limit: i64,
        offset: i64,
        query: Option<&str>,
    ) -> Result<Vec<GeneratedAccountRecord>, sqlx::Error> {
        if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
            let like = format!("%{}%", q.trim().to_lowercase());
            let sql = format!(
                "SELECT {}
                 FROM generated_accounts
                 WHERE lower(address) LIKE ? OR lower(status) LIKE ? OR lower(run_id) LIKE ?
                 ORDER BY created_at DESC
                 LIMIT ? OFFSET ?",
                Self::GENERATED_ACCOUNT_COLUMNS
            );
            let records = sqlx::query_as::<_, GeneratedAccountRecord>(&sql)
                .bind(&like)
                .bind(&like)
                .bind(&like)
                .bind(limit.clamp(1, 1000))
                .bind(offset.max(0))
                .fetch_all(&self.pool)
                .await?;
            Ok(records)
        } else {
            let sql = format!(
                "SELECT {}
                 FROM generated_accounts
                 ORDER BY created_at DESC
                 LIMIT ? OFFSET ?",
                Self::GENERATED_ACCOUNT_COLUMNS
            );
            let records = sqlx::query_as::<_, GeneratedAccountRecord>(&sql)
                .bind(limit.clamp(1, 1000))
                .bind(offset.max(0))
                .fetch_all(&self.pool)
                .await?;
            Ok(records)
        }
    }

    /// 获取全局账号总数（支持搜索）
    pub async fn count_all_accounts(&self, query: Option<&str>) -> Result<i64, sqlx::Error> {
        let count = if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
            let like = format!("%{}%", q.trim().to_lowercase());
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM generated_accounts WHERE lower(address) LIKE ? OR lower(status) LIKE ? OR lower(run_id) LIKE ?"
            )
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM generated_accounts")
                .fetch_one(&self.pool)
                .await?
        };
        Ok(count)
    }

    /// 获取所有符合条件的账号 ID
    pub async fn list_all_account_ids(
        &self,
        query: Option<&str>,
    ) -> Result<Vec<String>, sqlx::Error> {
        let ids = if let Some(q) = query.filter(|s| !s.trim().is_empty()) {
            let like = format!("%{}%", q.trim().to_lowercase());
            let rows = sqlx::query(
                "SELECT id FROM generated_accounts WHERE lower(address) LIKE ? OR lower(status) LIKE ? OR lower(run_id) LIKE ? ORDER BY created_at DESC"
            )
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_all(&self.pool)
            .await?;

            rows.into_iter()
                .map(|r| {
                    use sqlx::Row;
                    r.get::<String, _>("id")
                })
                .collect()
        } else {
            let rows = sqlx::query("SELECT id FROM generated_accounts ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?;

            rows.into_iter()
                .map(|r| {
                    use sqlx::Row;
                    r.get::<String, _>("id")
                })
                .collect()
        };
        Ok(ids)
    }

    /// 删除指定的已生成账号产物
    pub async fn delete_generated_account(&self, id: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM generated_accounts WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// 批量删除已生成的账号产物
    pub async fn delete_generated_accounts(&self, ids: &[String]) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }

        let placeholders = vec!["?"; ids.len()].join(", ");
        let sql = format!("DELETE FROM generated_accounts WHERE id IN ({placeholders})");
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }

        let result = query.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }
    /// 清理所有注册失败（状态不包含 registered 或 success）的账号记录
    pub async fn delete_failed_accounts(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM generated_accounts 
             WHERE status NOT LIKE '%registered%' 
             AND LOWER(status) NOT LIKE '%success%'",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// 更新账号状态
    pub async fn update_account_status(&self, id: &str, status: &str) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("UPDATE generated_accounts SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// 更新账号网关运行活动与连续失败次数记录，并在连续失败 3 次以上时执行熔断隔离断路逻辑
    pub async fn update_account_gateway_activity(
        &self,
        id: &str,
        consecutive_failures: i64,
        last_failure_reason: Option<&str>,
    ) -> Result<u64, sqlx::Error> {
        let mut status_clause = "".to_string();
        let mut new_status = None;
        if consecutive_failures >= 3 {
            status_clause = ", status = 'Zombie'".to_string();
            new_status = Some("Zombie");
        }

        let sql = format!(
            "UPDATE generated_accounts 
             SET consecutive_failures = ?, last_failure_reason = ? {status_clause}
             WHERE id = ?"
        );

        let result = sqlx::query(&sql)
            .bind(consecutive_failures)
            .bind(last_failure_reason)
            .bind(id)
            .execute(&self.pool)
            .await?;

        if let Some(ns) = new_status {
            println!(
                "🚨 [熔断器] 账号 {id} 因连续遭遇 {consecutive_failures} 次请求失败，已被自动熔断隔离并标记为 '{ns}'！"
            );
        }

        Ok(result.rows_affected())
    }

    /// 标记账号进入速率限制冷却状态
    pub async fn mark_account_cooling_down(
        &self,
        id: &str,
        cool_down_duration_secs: i64,
    ) -> Result<u64, sqlx::Error> {
        let reset_at = Utc::now().timestamp() + cool_down_duration_secs;
        let result = sqlx::query(
            "UPDATE generated_accounts 
             SET rate_limit_reset_at = ? 
             WHERE id = ?",
        )
        .bind(reset_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// 修改指定账号的分组标签 (pool_tag)
    pub async fn update_account_pool_tag(
        &self,
        id: &str,
        pool_tag: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE generated_accounts 
             SET pool_tag = ? 
             WHERE id = ?",
        )
        .bind(pool_tag)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Atomically lease one routable account so concurrent requests cannot select the same oldest row.
    pub async fn lease_active_account_for_routing(
        &self,
        pool_tag: &str,
    ) -> Result<Option<GeneratedAccountRecord>, sqlx::Error> {
        let now = Utc::now().timestamp();
        let sql = format!(
            "UPDATE generated_accounts
             SET last_used_at = MAX(COALESCE(last_used_at, 0) + 1, ?),
                 request_count_24h = COALESCE(request_count_24h, 0) + 1
             WHERE id = (
                 SELECT id
                 FROM generated_accounts
                 WHERE (status LIKE '%registered%' OR lower(status) LIKE '%success%')
                   AND (pool_tag = ? OR (? = 'default' AND pool_tag IS NULL))
                   AND (rate_limit_reset_at IS NULL OR rate_limit_reset_at <= ?)
                   AND access_token IS NOT NULL AND access_token != ''
                   AND (proxy_status IS NULL OR proxy_status != 'offline')
                 ORDER BY
                   (CASE WHEN proxy_status = 'active' THEN 0 WHEN proxy_status IS NULL THEN 1 ELSE 2 END) ASC,
                   (CASE WHEN proxy_ip_type = 'residential' THEN 0 ELSE 1 END) ASC,
                   proxy_rtt ASC,
                   last_used_at ASC,
                   created_at DESC
                 LIMIT 1
             )
             RETURNING {}",
            Self::GENERATED_ACCOUNT_COLUMNS
        );

        sqlx::query_as::<_, GeneratedAccountRecord>(&sql)
            .bind(now)
            .bind(pool_tag)
            .bind(pool_tag)
            .bind(now)
            .fetch_optional(&self.pool)
            .await
    }
    /// 获取单个生成的账号产物
    pub async fn get_generated_account(
        &self,
        id: &str,
    ) -> Result<Option<GeneratedAccountRecord>, sqlx::Error> {
        let sql = format!(
            "SELECT {}
             FROM generated_accounts
             WHERE id = ?
             LIMIT 1",
            Self::GENERATED_ACCOUNT_COLUMNS
        );
        let record = sqlx::query_as::<_, GeneratedAccountRecord>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(record)
    }

    /// 内部 OTP 轮询：根据收件地址查询最近的验证码
    pub async fn poll_otp_by_email(
        &self,
        email: &str,
        since_ts: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        use sqlx::Row;

        let email_lower = email.trim().to_lowercase();

        let rows = sqlx::query(
            "SELECT extracted_code FROM emails
             WHERE to_addr = ? AND extracted_code IS NOT NULL AND extracted_code != ''
               AND created_at >= ?
             ORDER BY created_at DESC
             LIMIT 10",
        )
        .bind(&email_lower)
        .bind(since_ts)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().find_map(|row| {
            let raw = row.get::<String, _>("extracted_code");
            let normalized = raw
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .collect::<String>()
                .to_ascii_uppercase();
            (normalized.len() == 6
                && normalized
                    .chars()
                    .any(|character| character.is_ascii_digit()))
            .then_some(normalized)
        }))
    }

    /// 内部链接轮询：根据收件地址查询最近的验证链接
    pub async fn poll_link_by_email(
        &self,
        email: &str,
        since_ts: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        use sqlx::Row;

        let email_lower = email.trim().to_lowercase();

        let row = sqlx::query(
            "SELECT extracted_link FROM emails
             WHERE to_addr = ? AND extracted_link IS NOT NULL AND extracted_link != ''
               AND created_at >= ?
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(&email_lower)
        .bind(since_ts)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get("extracted_link")))
    }

    /// 获取某次工作流的步骤详情
    pub async fn list_workflow_steps(
        &self,
        run_id: &str,
        limit: i64,
    ) -> Result<Vec<WorkflowStepRecord>, sqlx::Error> {
        let records = sqlx::query_as::<_, WorkflowStepRecord>(
            "SELECT 
                s.id, s.run_id, s.step_index, s.level, s.message, s.created_at,
                r.workflow_id, r.workflow_title
             FROM workflow_run_steps s
             JOIN workflow_runs r ON s.run_id = r.id
             WHERE s.run_id = ?
             ORDER BY s.step_index ASC, s.created_at ASC
             LIMIT ?",
        )
        .bind(run_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// 写入或更新应用配置项
    pub async fn upsert_setting(&self, key: &str, value: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"
        )
        .bind(key)
        .bind(value)
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// 读取全部应用配置
    pub async fn list_settings(&self) -> Result<HashMap<String, String>, sqlx::Error> {
        use sqlx::Row;

        let rows = sqlx::query("SELECT key, value FROM app_settings")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| (row.get("key"), row.get("value")))
            .collect())
    }

    /// 按键读取单个配置项
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, sqlx::Error> {
        use sqlx::Row;

        let row = sqlx::query("SELECT value FROM app_settings WHERE key = ? LIMIT 1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|value| value.get("value")))
    }

    pub async fn delete_setting(&self, key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM app_settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_admin_credentials(&self) -> Result<Option<AdminCredentials>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT key, value FROM app_settings WHERE key IN ('admin_username', 'admin_password_hash')",
        )
        .fetch_all(&self.pool)
        .await?;
        let values: HashMap<String, String> = rows
            .into_iter()
            .map(|row| (row.get("key"), row.get("value")))
            .collect();
        match (
            values
                .get("admin_username")
                .filter(|value| !value.trim().is_empty()),
            values
                .get("admin_password_hash")
                .filter(|value| !value.trim().is_empty()),
        ) {
            (Some(username), Some(password_hash)) => Ok(Some(AdminCredentials {
                username: username.clone(),
                password_hash: password_hash.clone(),
            })),
            _ => Ok(None),
        }
    }

    pub async fn replace_admin_credentials(
        &self,
        username: &str,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().timestamp();
        let mut transaction = self.pool.begin().await?;
        for (key, value) in [
            ("admin_username", username),
            ("admin_password_hash", password_hash),
        ] {
            sqlx::query(
                "INSERT INTO app_settings (key, value, updated_at)
                 VALUES (?, ?, ?)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// 将 Webhook 地址设置为当前活跃地址，避免重复插入
    pub async fn upsert_webhook(&self, url: &str) -> Result<(), sqlx::Error> {
        use sqlx::Row;

        let now = Utc::now().timestamp();
        let exists = sqlx::query("SELECT id FROM webhooks WHERE url = ? LIMIT 1")
            .bind(url)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = exists {
            let id: String = row.get("id");
            sqlx::query("UPDATE webhooks SET is_active = 1, created_at = ? WHERE id = ?")
                .bind(now)
                .bind(id)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query("INSERT INTO webhooks (id, url, created_at) VALUES (?, ?, ?)")
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(url)
                .bind(now)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::DataLake;
    use std::sync::Arc;

    fn temp_db_url(name: &str) -> (String, std::path::PathBuf) {
        let mut path = std::env::temp_dir();
        path.push(format!("phantomdrop_{name}_{}.db", uuid::Uuid::new_v4()));
        (
            format!(
                "sqlite://{}?mode=rwc",
                path.to_string_lossy().replace('\\', "/")
            ),
            path,
        )
    }

    async fn new_test_lake(name: &str) -> (Arc<DataLake>, std::path::PathBuf) {
        let (url, path) = temp_db_url(name);
        (DataLake::new(&url).await, path)
    }

    #[tokio::test]
    async fn legacy_duplicate_workflow_steps_are_cleaned_before_unique_index() {
        let (url, path) = temp_db_url("legacy_duplicate_steps");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("create legacy database");
        sqlx::query(
            "CREATE TABLE workflow_run_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create legacy steps table");
        for id in ["step-oldest", "step-duplicate"] {
            sqlx::query(
                "INSERT INTO workflow_run_steps
                 (id, run_id, step_index, level, message, created_at)
                 VALUES (?, 'run-1', 1, 'info', 'legacy', 1)",
            )
            .bind(id)
            .execute(&pool)
            .await
            .expect("insert duplicate legacy step");
        }
        pool.close().await;

        let lake = DataLake::new(&url).await;
        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workflow_run_steps WHERE run_id = 'run-1' AND step_index = 1",
        )
        .fetch_one(&lake.pool)
        .await
        .expect("count upgraded steps");
        assert_eq!(remaining, 1);
        drop(lake);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn archived_email_can_be_queried_from_page_endpoint() {
        let (lake, path) = new_test_lake("archived_page").await;

        lake.record_email(
            "email-1",
            "from@example.com",
            "to@example.com",
            "subject",
            "body",
            "",
            Some("123456"),
            None,
            None,
            None,
        )
        .await
        .expect("写入测试邮件失败");

        lake.set_email_archived("email-1", true)
            .await
            .expect("归档测试邮件失败");

        let page = lake
            .get_emails_page(1, 10, None, Some(true))
            .await
            .expect("分页查询归档邮件失败");

        assert_eq!(page.total, 1);
        assert_eq!(page.items.len(), 1);
        assert!(page.items[0].is_archived);

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn email_and_webhook_outbox_share_a_transaction() {
        let (lake, path) = new_test_lake("webhook_outbox").await;
        lake.upsert_webhook("https://example.com/hook")
            .await
            .expect("create webhook");

        lake.record_email(
            "email-outbox-1",
            "from@example.com",
            "to@example.com",
            "subject",
            "body",
            "",
            None,
            None,
            None,
            Some(r#"{"type":"EMAIL_INGEST_READY"}"#),
        )
        .await
        .expect("record email and webhook job");

        let delivery = lake
            .lease_webhook_delivery()
            .await
            .expect("lease outbox")
            .expect("outbox job");
        assert_eq!(delivery.webhook_url, "https://example.com/hook");
        assert_eq!(delivery.attempts, 0);

        lake.fail_webhook_delivery(&delivery.id, delivery.attempts, "temporary")
            .await
            .expect("reschedule delivery");
        assert!(
            lake.lease_webhook_delivery()
                .await
                .expect("lease delayed")
                .is_none()
        );

        sqlx::query(
            "UPDATE webhook_outbox SET status = 'pending', next_attempt_at = 0 WHERE id = ?",
        )
        .bind(&delivery.id)
        .execute(&lake.pool)
        .await
        .expect("make delivery due");
        let retry = lake
            .lease_webhook_delivery()
            .await
            .expect("lease retry")
            .expect("retry job");
        assert_eq!(retry.attempts, 1);
        lake.complete_webhook_delivery(&retry.id)
            .await
            .expect("complete delivery");
        assert!(
            lake.lease_webhook_delivery()
                .await
                .expect("empty outbox")
                .is_none()
        );

        drop(lake);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn running_workflows_are_marked_interrupted_on_recovery() {
        let (lake, path) = new_test_lake("workflow_recovery").await;
        lake.create_workflow_run("run-1", "workflow-1", "Workflow", "running", "started")
            .await
            .expect("create workflow run");
        assert_eq!(
            lake.recover_interrupted_workflow_runs()
                .await
                .expect("recover workflow runs"),
            1
        );
        assert_eq!(
            lake.get_workflow_run_status("run-1")
                .await
                .expect("read workflow status"),
            "interrupted"
        );
        drop(lake);
        let _ = std::fs::remove_file(path);
    }
}
