# 幻影中枢

幻影中枢是一个以邮件接收、验证码提取、实时事件流和自动化工作流为核心的本地中枢项目。

## 目录结构

- `core/`：Rust 后端与内建控制台，提供接口、实时事件流、工作流执行与数据存储。
- `web/`：前端界面，用于查看邮件、日志、内网穿透状态、自动化工作流和系统设置。
- `network/`：边缘邮件转发节点，用于把外部邮件事件转发到中枢。
- `tools/`：项目辅助工具。

## 核心能力

- 实时接收邮件并提取验证码、链接和正文信息。
- 提供工作流定义、执行记录、步骤轨迹与任务产物查询。
- 提供内网穿透登记页，允许登记公网地址供外部节点接入。
- 提供系统设置页，集中维护密钥、回调地址、域名和自动化配置。
- **单镜像交付**：前后端整合打包，后端原生托管前端 SPA，一键部署。

## 快速启动 (Docker/生产模式)

推荐使用 Docker 进行一键部署，镜像中已内置完整的自动化执行环境。

1. **准备环境**：
   复制并配置环境变量：`cp .env.example .env`
2. **启动项目**：
   ```bash
   docker-compose up -d
   ```
3. **访问地址**：
   `http://localhost:4000`

## 快速启动 (本地开发模式)

### 启动全栈环境

在项目根目录执行以下 PowerShell 脚本，将同时启动前端 Vite 开发服务器和后端 Rust 服务：

```powershell
.\启动开发环境.ps1
```

默认地址：
- 前端控制台：`http://127.0.0.1:5173`
- 后端 API 基址：`http://127.0.0.1:4000`

### 独立启动后端

```bash
cd core
cargo run --release
```

## 部署说明

### 环境变量配置

请参考 `.env.example` 进行配置：

- `auth_secret`: API 接口令牌，请在 Web 控制台“全局设置”中配置。
- `HUB_SECRET`: 可选兜底环境变量；Docker 生产环境通常不需要配置。
- `PHANTOM_DB_URL`: SQLite 数据库连接字符串（容器内建议保持默认）。
- `CLOUDFLARE_API_TOKEN`: 用于驱动自动化脚本的 Cloudflare 令牌。
- `WEB_DIST`: (可选) 前端静态产物目录。

生产环境请设置 `APP_ENV=production`，并在 Web 控制台“全局设置”中配置接口令牌。Cloudflare Worker 侧不要把 `HUB_SECRET` 写入 `wrangler.toml`，请使用 `npx wrangler secret put HUB_SECRET` 注入同一个令牌。
Worker 的 `/health` 会检查 `PHANTOM_HUB_URL` 和 `HUB_SECRET` 是否可用；`npm run dry-run` 会调用 Wrangler 并可能与 Cloudflare 通信。

### 数据持久化

Docker 部署时，请务必挂载以下两个目录以保证数据不丢失：
- `./data`: 存储 SQLite 数据库。
- `./.automation`: 存储自动化任务状态和日志。

## 开发约定

- 代码注释、界面文案和说明文档统一使用简体中文。
- 改动界面文案时，不修改接口路径、环境变量键名和外部协议字段。
- 提交前清理无关调试输出。
- 编码统一使用 UTF-8，避免中文乱码。

## 质量验证

本地验证命令见 `VERIFY.md`。仓库已提供 GitHub Actions 工作流，覆盖 Rust 后端测试、Web 生产构建和 Worker 类型检查。

## 专题文档

- `docs/OAuth认证提取与导出说明.md`：记录 OpenAI OAuth 凭证提取、入库、兜底合并和批量导出的完整链路。
