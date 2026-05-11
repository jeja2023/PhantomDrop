# OAuth 认证提取与导出说明

本文记录 PhantomDrop 在账号注册成功后提取、保存和导出 OpenAI OAuth 凭证的当前实现。

## 凭证来源

- 协议注册流程会在 OAuth token exchange 成功后读取 `access_token`、`refresh_token` 与 `id_token`。
- 浏览器注册流程会扫描会话接口、浏览器存储和页面内嵌数据，区分 API Access Token 与客户端 ID Token。
- 手动更新 token 接口会先读取数据库旧值，再合并本次提交的新值，避免空字符串覆盖已有凭证。

## 入库字段

`generated_accounts` 额外保存以下 OAuth 字段：

- `id_token`
- `chatgpt_account_id`
- `chatgpt_user_id`
- `organization_id`
- `plan_type`
- `expires_in`
- `token_version`
- `oauth_credentials_json`

旧数据库启动时会通过补列逻辑自动补齐这些字段。

## 统一兜底规则

OAuth 凭证由 `core/src/openai/oauth.rs` 中的统一构建器生成。优先级如下：

1. 使用本次注册或接口提交的真实字段。
2. 使用历史 `oauth_credentials_json` 中已经保存的字段。
3. 从 `id_token` 的 JWT Payload 中解析 OpenAI Auth 命名空间。
4. 使用邮箱、access token 或 workspace id 生成稳定兜底值。

当真实 `id_token` 缺失时，系统会生成稳定的 mock ID Token。该 token 只用于保持导出结构完整，不代表已通过 OpenAI 签名验签。

## 导出结构

账号中心的 `OAuth JSON` 导出会生成如下结构：

```json
{
  "exported_at": "2026-05-11T00:00:00Z",
  "proxies": [],
  "accounts": [
    {
      "name": "user@example.com",
      "platform": "openai",
      "type": "oauth",
      "credentials": {
        "_token_version": 1778215057457,
        "access_token": "...",
        "chatgpt_account_id": "...",
        "chatgpt_user_id": "...",
        "email": "user@example.com",
        "expires_in": 864000,
        "id_token": "...",
        "organization_id": "...",
        "plan_type": "free",
        "refresh_token": "..."
      },
      "extra": {
        "email": "user@example.com",
        "privacy_mode": "training_off"
      },
      "concurrency": 10,
      "priority": 1,
      "rate_multiplier": 1,
      "auto_pause_on_expired": true
    }
  ]
}
```

## 前端展示

账号详情弹窗会展示：

- Access Token
- Session Token
- Refresh Token
- ID Token
- ChatGPT Account ID
- ChatGPT User ID
- Organization ID
- Plan Type
- Expires In
- Token Version
- OAuth Credentials JSON

弹窗通过 React Portal 挂载到 `document.body`，避免被侧边栏、顶部栏或局部滚动容器遮挡。

## 验证命令

```powershell
cd D:\project\PhantomDrop\core
cargo fmt --check
cargo check --target-dir target-oauth-check-20260511
cargo test openai::oauth --target-dir target-oauth-check-20260511 -- --nocapture

cd D:\project\PhantomDrop\web
npm run build
```
