# 本地验证指南

在项目根目录执行以下 `V0.0.34` 质量门禁。

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

## Docker

```powershell
$env:ADMIN_USERNAME = 'verify-admin'
$env:ADMIN_PASSWORD = 'verify-password-1234'
docker compose config --quiet
docker build --tag phantom-drop:0.0.34 .
```

`HUB_SECRET` 在 Docker 验证中是可选项。只有需要验证 Worker 邮件接入时才设置。