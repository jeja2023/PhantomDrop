# PhantomDrop 幻影中枢

PhantomDrop 是一个以邮件接收、验证码提取、实时事件流和自动化工作流为核心的本地中枢。当前版本为 `V0.0.33`。

## 目录结构

- `core/`：Rust 后端、SQLite 数据层和内建控制台。
- `web/`：React 管理界面。
- `network/`：Cloudflare 邮件转发 Worker。
- `tools/`：发布与辅助工具。

## 认证模型

管理端与机器通信使用两套完全独立的凭据：

- `ADMIN_USERNAME` / `ADMIN_PASSWORD`：仅用于首次初始化管理员账户。密码经 Argon2id 加盐哈希后写入 SQLite，浏览器登录后使用 `HttpOnly`、`SameSite=Strict` Cookie；管理 API 不接受原始密码、Bearer 密钥或 `X-Auth-Token`。
- `HUB_SECRET`：仅用于 Cloudflare Worker 调用 Hub 的 `/ingest`，不能登录管理端，也不会由管理设置 API 返回或写入自动化 JSON。

`HUB_SECRET` 不是运行核心控制台的必需项。不使用 Cloudflare Email Worker 时可以留空，此时 `/ingest` 返回 `503`，其他功能正常运行。启用 Worker 邮件接入时，Hub 和 Worker 必须配置相同的强随机 `HUB_SECRET`。

## Docker 部署

1. 从 `.env.example` 创建 `.env`。
2. 首次启动填写 `ADMIN_USERNAME` 和至少 12 个字符的 `ADMIN_PASSWORD`。
3. 仅在启用 Worker 邮件接入时填写强随机 `HUB_SECRET`。
4. 执行 `docker compose up -d --build`。
5. 打开 `http://localhost:9010`，使用管理员用户名和密码登录。

管理员凭据初始化成功后保存在持久化 SQLite 数据库中。后续可在“系统设置与维护 -> 账户与登录”修改用户名或密码；修改后旧会话立即失效。从 `V0.0.31` 及更早版本升级且数据库尚无管理员时，第一次启动 `V0.0.33` 必须提供 `ADMIN_PASSWORD`；已在 `V0.0.32` 完成管理员初始化的数据库无需再次提供。旧 `auth_secret` 会在机器密钥校验通过后删除，且不会迁移为管理员密码。

容器以非 root 用户运行，SQLite 与自动化状态分别保存在 `data`、`automation` 命名卷。HTTPS 反向代理部署时设置 `COOKIE_SECURE=true`；本地 HTTP 访问保持 `false`。

## 从旧版本升级生产环境

可以原地升级到 `V0.0.33`，但必须使用现有 `data` 和 `automation` 命名卷，且先完成备份和认证迁移：

1. 停止旧容器，避免备份时 SQLite WAL 仍在写入：`docker compose stop phantom-drop`。
2. 备份数据卷：`docker run --rm -v phantomdrop_data:/source:ro -v ${PWD}:/backup alpine sh -c "cd /source && tar czf /backup/phantomdrop-data-before-v0.0.33.tgz ."`。卷名以 `docker volume ls` 的实际结果为准。
3. 若旧数据库尚无管理员，在生产 `.env` 设置新的 `ADMIN_USERNAME` 与至少 12 个字符的 `ADMIN_PASSWORD`；已完成管理员初始化则保留数据库凭据，无需再次注入。
4. 如果旧部署启用了 Worker，把旧数据库 `auth_secret` 的同一值设置为 `HUB_SECRET`。首次升级若缺失或不一致，服务会拒绝启动且保留旧值，避免 Worker 静默断开。
5. 更新代码后执行 `docker compose up -d --build`，再检查 `docker compose logs --tail=200 phantom-drop` 和 `/health`。
6. 使用管理员用户名/密码登录，确认邮件、工作流和账号数据正常。管理员创建成功后，后续改密在“账户与登录”完成。
7. 需要轮换机器密钥时，再设置新的 `HUB_SECRET` 并执行 Cloudflare 自动化，将同一值同步到 Worker Secret Store。

启动时会自动执行 SQLx 迁移。旧库若有重复的工作流步骤，升级预处理会保留每组最早记录后再创建唯一索引。不要用新的空卷替换现有 `data` 卷，否则看起来会像数据丢失。

## 关键配置

- `ADMIN_USERNAME`：首次初始化用户名，默认 `admin`。
- `ADMIN_PASSWORD`：首次初始化密码，至少 12 个字符；已有管理员记录时不用于覆盖数据库凭据。
- `HUB_SECRET`：可选的 Worker -> Hub 机器通信密钥。
- `CORS_ORIGINS`：允许携带认证 Cookie 的来源白名单，生产环境不得使用 `*`。
- `PHANTOM_GATEWAY_KEYS`：OpenAI 网关密钥到账号池的映射，格式为 `key=pool,key2=pool2`。
- `MAX_CONCURRENT_WORKFLOWS`：工作流并发上限，默认 `2`。
- `MAX_CONCURRENT_PROXY_CHECKS`：代理检测并发上限，默认 `8`。
- `ENABLE_DEBUG_ASSETS`：调试截图开关，生产环境保持 `false`。

Cloudflare Worker 的密钥使用 `npx wrangler secret put HUB_SECRET` 注入，不要写入 `wrangler.toml`。自动化脚本从 Hub 进程环境继承 `HUB_SECRET`，并同步到 Cloudflare Secret Store。

## 本地开发

日常开发使用持久化数据库 `core/phantom_core.db`，不是 `.codex-tmp` 中的一次性预览库。首次启动空库时设置管理员初始化变量；成功创建管理员后，用户名和 Argon2id 密码哈希保存在该数据库中，后续启动无需继续提供初始化变量。需要隔离测试时才使用 `.codex-tmp` 或 `artifacts` 下的临时数据库。

```powershell
# 后端
$env:ADMIN_USERNAME = 'admin'
$env:ADMIN_PASSWORD = 'change-this-local-password'
cd core
cargo run

# 前端（另一个终端）
cd web
npm ci
npm run dev
```

如果 `core/phantom_core.db` 已完成初始化，可先执行 `Remove-Item Env:ADMIN_USERNAME, Env:ADMIN_PASSWORD -ErrorAction SilentlyContinue` 再启动；环境变量不会覆盖已有管理员。开发库与生产命名卷是两套独立数据源，生产升级必须复用并备份原有 `data` 卷。

默认前端地址为 `http://127.0.0.1:5173`，后端与单镜像入口为 `http://127.0.0.1:9010`。完整质量门禁和认证冒烟测试见 `VERIFY.md`，详细发布内容见 `更新日志.md`。