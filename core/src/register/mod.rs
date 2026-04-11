use crate::db::DataLake;
use std::sync::Arc;

/**
 * 通用注册辅助模块
 * 提供账号持久化、状态同步及多平台分发逻辑的抽象
 */

pub struct RegistrationManager {
    dl: Arc<DataLake>,
}

impl RegistrationManager {
    pub fn new(dl: Arc<DataLake>) -> Self {
        Self { dl }
    }

    /// 将注册成功的产物统一写入生成的账号表
    pub async fn persist_account(
        &self,
        run_id: &str,
        address: &str,
        password: &str,
        status: &str,
    ) -> Result<(), String> {
        self.dl
            .create_generated_account(run_id, address, password, status)
            .await
            .map(|_| ())
            .map_err(|e| format!("账号持久化失败: {:?}", e))
    }

    /// 标记注册任务的中间状态
    pub async fn update_registration_status(
        &self,
        _run_id: &str,
        _status: &str,
    ) -> Result<(), String> {
        // 未来可以扩展专门的注册任务追踪表
        Ok(())
    }
}
