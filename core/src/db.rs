use chrono::Utc;
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};
use std::collections::HashMap;
use std::sync::Arc;

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
    pub device_id: Option<String>,
    pub workspace_id: Option<String>,
    pub upload_status: Option<String>,
    pub account_type: Option<String>,
    pub proxy_url: Option<String>,
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
    pub latest_email_at: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct EmailPage {
    pub items: Vec<EmailRecord>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

pub struct DataLake {
    pub pool: Pool<Sqlite>,
}

impl DataLake {
    /// 初始化数据湖连接并确保表结构存在
    pub async fn new(database_url: &str) -> Arc<Self> {
        // 使用高性能连接池
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .expect("无法连接到 SQLite 数据湖");

        // 基础表结构迁移
        Self::ensure_tables(&pool).await;

        Arc::new(Self { pool })
    }

    async fn ensure_tables(pool: &Pool<Sqlite>) {
        // 创建核心邮件存储表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS emails (
                id TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                from_addr TEXT NOT NULL,
                to_addr TEXT NOT NULL,
                subject TEXT,
                body_text TEXT,
                body_html TEXT,
                extracted_code TEXT,
                extracted_link TEXT,
                extracted_text TEXT,
                is_archived BOOLEAN DEFAULT FALSE
            )",
        )
        .execute(pool)
        .await
        .expect("数据表初始化失败");

        // 创建 Webhook 订阅表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS webhooks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                event_filter TEXT DEFAULT '*',
                is_active BOOLEAN DEFAULT TRUE,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(pool)
        .await
        .expect("Webhook表初始化失败");

        // 创建应用级配置表，用于替代前端本地运行期依赖
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(pool)
        .await
        .expect("应用配置表初始化失败");

        // 创建工作流执行记录表
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS workflow_runs (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                workflow_title TEXT NOT NULL,
                status TEXT NOT NULL,
                message TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                finished_at INTEGER
            )",
        )
        .execute(pool)
        .await
        .expect("工作流执行记录表初始化失败");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS workflow_definitions (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL DEFAULT 'account_generate',
                title TEXT NOT NULL,
                summary TEXT NOT NULL,
                status TEXT NOT NULL,
                parameters_json TEXT NOT NULL DEFAULT '{}',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )
        .execute(pool)
        .await
        .expect("工作流定义表初始化失败");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS workflow_run_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
        )
        .execute(pool)
        .await
        .expect("工作流步骤记录表初始化失败");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS generated_accounts (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                address TEXT NOT NULL,
                password TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                access_token TEXT,
                refresh_token TEXT,
                session_token TEXT,
                device_id TEXT,
                workspace_id TEXT,
                upload_status TEXT DEFAULT 'pending',
                account_type TEXT,
                proxy_url TEXT
            )",
        )
        .execute(pool)
        .await
        .expect("生成账号表初始化失败");

        // 尝试添加新字段，如果由于表已存在而缺少字段的话 (通过静默忽略错误来简单处理增量更新)
        let _ = sqlx::query("ALTER TABLE emails ADD COLUMN extracted_link TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE emails ADD COLUMN extracted_text TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE workflow_definitions ADD COLUMN kind TEXT NOT NULL DEFAULT 'account_generate'").execute(pool).await;

        // 生成账号表增量迁移：补充 Token 及分发字段
        let _ = sqlx::query("ALTER TABLE generated_accounts ADD COLUMN access_token TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE generated_accounts ADD COLUMN refresh_token TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE generated_accounts ADD COLUMN session_token TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE generated_accounts ADD COLUMN device_id TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE generated_accounts ADD COLUMN workspace_id TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query(
            "ALTER TABLE generated_accounts ADD COLUMN upload_status TEXT DEFAULT 'pending'",
        )
        .execute(pool)
        .await;
        let _ = sqlx::query("ALTER TABLE generated_accounts ADD COLUMN account_type TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE generated_accounts ADD COLUMN proxy_url TEXT")
            .execute(pool)
            .await;

        // 创建索引加速查询 (特别是针对收件地址的实时过滤)
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_to_addr ON emails (to_addr)")
            .execute(pool)
            .await
            .expect("索引创建失败");

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_workflow_runs_started_at ON workflow_runs (started_at DESC)")
            .execute(pool)
            .await
            .expect("工作流索引创建失败");

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_workflow_definitions_updated_at ON workflow_definitions (updated_at DESC)")
            .execute(pool)
            .await
            .expect("工作流定义索引创建失败");

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_workflow_run_steps_run_id ON workflow_run_steps (run_id, step_index)")
            .execute(pool)
            .await
            .expect("工作流步骤索引创建失败");

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_generated_accounts_run_id ON generated_accounts (run_id, created_at DESC)")
            .execute(pool)
            .await
            .expect("生成账号索引创建失败");
    }

    /// 插入一条新解析的原始邮件
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
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO emails (id, created_at, from_addr, to_addr, subject, body_text, body_html, extracted_code, extracted_link, extracted_text) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id)
        .bind(now)
        .bind(from)
        .bind(to)
        .bind(subject)
        .bind(text)
        .bind(html)
        .bind(extracted_code)
        .bind(extracted_link)
        .bind(extracted_text)
        .execute(&self.pool)
        .await?;

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
                (SELECT MAX(created_at) FROM emails) AS latest_email_at"
        )
        .bind(threshold_24h)
        .bind(threshold_24h)
        .bind(threshold_24h)
        .bind(threshold_24h)
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
            eprintln!("🔴 [数据库错误] 无法插入生成的账号: {:?}", e);
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
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE generated_accounts
             SET access_token = ?, refresh_token = ?, session_token = ?,
                 device_id = ?, workspace_id = ?
             WHERE id = ?",
        )
        .bind(access_token)
        .bind(refresh_token)
        .bind(session_token)
        .bind(device_id)
        .bind(workspace_id)
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

    /// 获取某次运行生成的账号产物
    pub async fn list_generated_accounts(
        &self,
        run_id: &str,
        limit: i64,
    ) -> Result<Vec<GeneratedAccountRecord>, sqlx::Error> {
        let sql = if run_id == "all" {
            "SELECT id, run_id, address, password, status, created_at,
                    access_token, refresh_token, session_token,
                    device_id, workspace_id, upload_status, account_type, proxy_url
             FROM generated_accounts
             ORDER BY created_at DESC
             LIMIT ?"
        } else {
            "SELECT id, run_id, address, password, status, created_at,
                    access_token, refresh_token, session_token,
                    device_id, workspace_id, upload_status, account_type, proxy_url
             FROM generated_accounts
             WHERE run_id = ?
             ORDER BY created_at DESC
             LIMIT ?"
        };

        let mut query = sqlx::query_as::<_, GeneratedAccountRecord>(sql);
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
            let records = sqlx::query_as::<_, GeneratedAccountRecord>(
                "SELECT id, run_id, address, password, status, created_at,
                        access_token, refresh_token, session_token,
                        device_id, workspace_id, upload_status, account_type, proxy_url
                 FROM generated_accounts
                 WHERE lower(address) LIKE ? OR lower(status) LIKE ? OR lower(run_id) LIKE ?
                 ORDER BY created_at DESC
                 LIMIT ? OFFSET ?",
            )
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(limit.clamp(1, 1000))
            .bind(offset.max(0))
            .fetch_all(&self.pool)
            .await?;
            Ok(records)
        } else {
            let records = sqlx::query_as::<_, GeneratedAccountRecord>(
                "SELECT id, run_id, address, password, status, created_at,
                        access_token, refresh_token, session_token,
                        device_id, workspace_id, upload_status, account_type, proxy_url
                 FROM generated_accounts
                 ORDER BY created_at DESC
                 LIMIT ? OFFSET ?",
            )
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
    pub async fn list_all_account_ids(&self, query: Option<&str>) -> Result<Vec<String>, sqlx::Error> {
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
            
            rows.into_iter().map(|r| {
                use sqlx::Row;
                r.get::<String, _>("id")
            }).collect()
        } else {
            let rows = sqlx::query("SELECT id FROM generated_accounts ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await?;
                
            rows.into_iter().map(|r| {
                use sqlx::Row;
                r.get::<String, _>("id")
            }).collect()
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
             AND LOWER(status) NOT LIKE '%success%'"
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// 更新账号状态
    pub async fn update_account_status(
        &self,
        id: &str,
        status: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("UPDATE generated_accounts SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// 获取单个生成的账号产物
    pub async fn get_generated_account(
        &self,
        id: &str,
    ) -> Result<Option<GeneratedAccountRecord>, sqlx::Error> {
        let record = sqlx::query_as::<_, GeneratedAccountRecord>(
            "SELECT id, run_id, address, password, status, created_at,
                    access_token, refresh_token, session_token,
                    device_id, workspace_id, upload_status, account_type, proxy_url
             FROM generated_accounts
             WHERE id = ?
             LIMIT 1",
        )
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

        let row = sqlx::query(
            "SELECT extracted_code FROM emails
             WHERE to_addr = ? AND extracted_code IS NOT NULL AND extracted_code != ''
               AND created_at >= ?
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(email)
        .bind(since_ts)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.get("extracted_code")))
    }

    /// 内部链接轮询：根据收件地址查询最近的验证链接
    pub async fn poll_link_by_email(
        &self,
        email: &str,
        since_ts: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        use sqlx::Row;

        let row = sqlx::query(
            "SELECT extracted_link FROM emails
             WHERE to_addr = ? AND extracted_link IS NOT NULL AND extracted_link != ''
               AND created_at >= ?
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(email)
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
}
