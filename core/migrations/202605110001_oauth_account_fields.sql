-- OAuth 账号凭证字段扩展。
-- 注意：不要修改已经发布过的 202605100001 初始迁移，否则 sqlx 会因为 checksum 不一致触发 VersionMismatch。

ALTER TABLE generated_accounts ADD COLUMN id_token TEXT;
ALTER TABLE generated_accounts ADD COLUMN chatgpt_account_id TEXT;
ALTER TABLE generated_accounts ADD COLUMN chatgpt_user_id TEXT;
ALTER TABLE generated_accounts ADD COLUMN organization_id TEXT;
ALTER TABLE generated_accounts ADD COLUMN plan_type TEXT;
ALTER TABLE generated_accounts ADD COLUMN expires_in INTEGER;
ALTER TABLE generated_accounts ADD COLUMN token_version INTEGER;
ALTER TABLE generated_accounts ADD COLUMN oauth_credentials_json TEXT;
