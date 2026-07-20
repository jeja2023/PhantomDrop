# 本地验证指南

在项目根目录执行以下 `V0.0.36` 质量门禁。

## Rust 后端

```powershell
cd core
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
```

## Web 前端

```powershell
cd web
npm ci
npm run lint
npm run build
npm audit --omit=dev --audit-level=high
```

## Cloudflare Worker

```powershell
cd network
npm ci
npm run typecheck
npm audit --omit=dev --audit-level=high
```

`npm run dry-run` 会调用 Wrangler，并可能向 Cloudflare 传输打包元数据，只在允许外联时执行。

## PowerShell 自动化

```powershell
$tokens = $null
$errors = $null
[void][Management.Automation.Language.Parser]::ParseFile(
  (Resolve-Path '.\setup-cloudflare-mail.ps1'),
  [ref]$tokens,
  [ref]$errors
)
if ($errors.Count) { throw ($errors -join [Environment]::NewLine) }
```

对 `initialize-cloudflare-automation.ps1` 重复同样检查。中文脚本保留 UTF-8 BOM，以兼容 Windows PowerShell 5。

## 认证冒烟测试

使用一次性数据库启动服务，验证管理认证与机器认证相互隔离：

```powershell
$env:ADMIN_USERNAME = 'verify-admin'
$env:ADMIN_PASSWORD = 'verify-password-1234'
$env:HUB_SECRET = 'verify-machine-secret-5678'
$env:PHANTOM_DB_URL = 'sqlite://../artifacts/auth-smoke.sqlite3?mode=rwc'
$env:PORT = '9011'
$env:WEB_DIST = '..\web\dist'
cargo run --manifest-path .\core\Cargo.toml
```

另一个终端检查：未登录管理 API 返回 `401`；错误密码返回 `401`；正确用户名/密码设置 Cookie 后返回 `200`；直接使用 `HUB_SECRET` 登录仍返回 `401`；`/ingest` 仅接受正确的 `X-Hub-Secret`。删除 `HUB_SECRET` 后重启，`/health` 仍为 `200`，`/ingest` 返回 `503`。

## Grok 注册就绪检查

登录管理端后请求 `GET /api/workflows/grok/readiness`，确认：

- 工作流 `account_domain` 为空时会依次回退到系统 `account_domain` 和 `cloudflare_zone_domain`。
- 缺少有效收信域名、`HUB_SECRET`、代理格式错误或无可用 Solver/Chromium 时，`ready` 为 `false`。
- 公网 Hub、Cloudflare Zone 匹配和最近邮件记录属于提示性检查，异常时返回 `warn`，不会单独阻止运行。
- 自动化中心保存配置后刷新报告，点击启动时再次检查；存在 `fail` 项时不创建外部注册任务。

## Docker

```powershell
$env:ADMIN_USERNAME = 'verify-admin'
$env:ADMIN_PASSWORD = 'verify-password-1234'
docker compose config --quiet
docker build --tag phantom-drop:0.0.36 .
```

`HUB_SECRET` 在 Docker 验证中是可选项。只有需要验证 Worker 邮件接入时才设置。


确认展开后的持久化挂载仍指向预期目录：

```powershell
docker compose config | Select-String -Pattern '/app/data|/app/.automation'
```
