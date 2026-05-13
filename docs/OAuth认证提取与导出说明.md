# OAuth 认证提取与导出说明

本文记录 PhantomDrop 对 OpenAI OAuth 凭证的提取、保存、过滤和导出规则。

## 凭证来源

- 浏览器注册流程会在账号注册并登录后，尝试从会话接口、浏览器存储和页面注入数据中提取真实凭证。
- 手动更新 token 接口会先读取数据库旧值，再合并本次提交的新值，并统一经过 OAuth 构建器过滤。
- 协议注册流程如果没有捕获真实 OAuth callback code，会直接失败，不再生成模拟 access token、refresh token 或 id token。

## 入库字段

`generated_accounts` 会保存以下 OAuth 相关字段：

- `access_token`
- `refresh_token`
- `session_token`
- `id_token`
- `chatgpt_account_id`
- `chatgpt_user_id`
- `organization_id`
- `plan_type`
- `expires_in`
- `token_version`
- `oauth_credentials_json`

旧数据库启动时会通过补列逻辑自动补齐这些字段。

## 真实凭证过滤规则

OAuth 凭证由 `core/src/openai/oauth.rs` 中的统一构建器生成。当前规则如下：

1. `access_token` 必须是可解析 JWT，并且不能包含 `fallback_signature`、`mock_signature` 或 `compat_signature_for_cpa_parsing_only`。
2. `id_token` 必须是可解析 JWT，并且不能包含上述 mock/fallback 签名。
3. `refresh_token` 必须是 OpenAI OAuth refresh token 形态，目前要求以 `rt_` 开头。
4. 账号 ID、用户 ID、组织 ID 和套餐类型优先从真实 JWT 的 `https://api.openai.com/auth` 命名空间解析。
5. 只有存在真实 OAuth token 时，才会接受历史 `oauth_credentials_json` 或数据库中的元数据作为补充。
6. 当真实 token 缺失时，导出字段保持为空，不再生成稳定 mock ID、mock ID Token 或本地派生组织 ID。

## 完整凭证 JSON 保存条件

`oauth_credentials_json` 只有在以下三项同时存在且通过过滤时才会保存：

- `access_token`
- `refresh_token`
- `id_token`

这可以避免只有邮箱、派生账号 ID 或 mock token 的账号被误判为可用 OAuth 账号。

## 导出结构

账号中心的 `OAuth JSON` 导出仍保持兼容结构：

```json
{
  "exported_at": "2026-05-13T00:00:00Z",
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
        "refresh_token": "rt_..."
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

如果某个账号没有提取到真实 OAuth 凭证，`credentials` 中对应 token 和元数据会保持空字符串。导出方应将这类账号视为“凭证未完成”，而不是可用 OAuth 账号。

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
rustfmt --edition 2024 --check src\openai\oauth.rs src\openai\register.rs src\workflow.rs
cargo check
cargo test openai::oauth -- --nocapture

cd D:\project\PhantomDrop\web
npm run build
```

注意：在部分 Windows 沙箱环境中，`cargo check` 或 `cargo test` 可能因为无法写入 `core\target\debug\deps\*.rmeta` 返回 `os error 5`。这属于本地 target 目录权限问题，不代表源码语义一定失败。
