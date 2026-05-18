-- 增加自给自足高可用 API 网关及智能冷却等功能所需的账号表扩展字段。

ALTER TABLE generated_accounts ADD COLUMN pool_tag TEXT DEFAULT 'default';
ALTER TABLE generated_accounts ADD COLUMN last_used_at INTEGER DEFAULT 0;
ALTER TABLE generated_accounts ADD COLUMN rate_limit_reset_at INTEGER DEFAULT 0;
ALTER TABLE generated_accounts ADD COLUMN consecutive_failures INTEGER DEFAULT 0;
ALTER TABLE generated_accounts ADD COLUMN request_count_24h INTEGER DEFAULT 0;
ALTER TABLE generated_accounts ADD COLUMN last_failure_reason TEXT;
