# PhantomDrop 幻影中枢

PhantomDrop 是一个以邮件接收、验证码提取、实时事件流和自动化工作流为核心的本地中枢。当前版本为 `V0.0.37`。

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

管理员凭据初始化成功后保存在持久化 SQLite 数据库中。后续可在“系统设置与维护 -> 账户与登录”修改用户名或密码；修改后旧会话立即失效。从 `V0.0.31` 及更早版本直接升级到 `V0.0.37` 且数据库尚无管理员时，首次启动必须提供 `ADMIN_PASSWORD`；已在 `V0.0.32` 或之后版本完成管理员初始化的数据库无需再次提供。旧 `auth_secret` 会在机器密钥校验通过后删除，且不会迁移为管理员密码。

容器以非 root 用户运行，SQLite 与自动化状态分别保存在宿主机的 `./data`、`./.automation` 目录。可通过 `PHANTOM_DATA_PATH`、`PHANTOM_AUTOMATION_PATH` 指定其他持久化路径。HTTPS 反向代理部署时设置 `COOKIE_SECURE=true`；本地 HTTP 访问保持 `false`。

## 从旧版本升级生产环境

可以原地升级到 `V0.0.37`，但必须复用现有持久化目录，且先完成备份和认证迁移：

1. 停止旧容器，避免备份时 SQLite WAL 仍在写入：`docker compose stop phantom-drop`。
2. 备份 `./data` 和 `./.automation` 目录；至少完整保留 `phantom_core.db` 及同目录下可能存在的 `-wal`、`-shm` 文件。
3. 若旧数据库尚无管理员，在生产 `.env` 设置新的 `ADMIN_USERNAME` 与至少 12 个字符的 `ADMIN_PASSWORD`；已完成管理员初始化则保留数据库凭据，无需再次注入。
4. 如果旧部署启用了 Worker，把旧数据库 `auth_secret` 的同一值设置为 `HUB_SECRET`。首次升级若缺失或不一致，服务会拒绝启动且保留旧值，避免 Worker 静默断开。
5. 更新代码后执行 `docker compose up -d --build`，再检查 `docker compose logs --tail=200 phantom-drop` 和 `/health`。
6. 使用管理员用户名/密码登录，确认邮件、工作流和账号数据正常。管理员创建成功后，后续改密在“账户与登录”完成。
7. 需要轮换机器密钥时，再设置新的 `HUB_SECRET` 并执行 Cloudflare 自动化，将同一值同步到 Worker Secret Store。

启动时会自动执行 SQLx 迁移。旧库若有重复的工作流步骤，升级预处理会保留每组最早记录后再创建唯一索引。不要让 `PHANTOM_DATA_PATH` 指向新的空目录，否则看起来会像数据丢失。

若升级后页面为空：从 `V0.0.32` 或更早版本升级时，旧数据通常仍在项目的 `./data/phantom_core.db`，保持默认挂载并重建容器即可恢复；若数据写入过命名卷，先用 `docker volume ls` 找到原卷，再在 `.env` 中将 `PHANTOM_DATA_PATH` 临时设置为实际卷名。迁移前应停服，并完整保留数据库及其 `-wal`、`-shm` 文件。

若启动日志提示 `检测到旧 auth_secret`，不要删除数据库；从 `app_settings` 表读取旧值，将其设置为 `.env` 中的 `HUB_SECRET` 后重启。应用完成首次启动迁移后会自动删除旧字段；如果仍使用 Cloudflare Worker，`HUB_SECRET` 必须继续与 Worker Secret 保持一致。宿主机 bind mount 目录需要允许容器 UID `10001` 写入，V0.0.37 镜像会在启动时自动修正归属。

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

## Grok 自动注册

自动化中心提供内建的 `grok_register_default` 工作流，注册链路为：动态发现 xAI 注册参数、发送并轮询邮箱验证码、完成 Turnstile、提交注册资料、提取 Grok `sso` Cookie 并写入账号库。

使用前需完成以下配置：

1. 配置可公网收信的域名。系统按工作流 `account_domain`、系统 `account_domain`、`cloudflare_zone_domain` 的顺序选择，已有 Cloudflare Zone 时无需重复填写同一域名。
2. 在“自动化中心”打开独立的“Grok 注册中心”，检查域名、`HUB_SECRET`、公网 Hub、邮件历史、代理和 Solver 的就绪状态，再配置批次、并发和与注册请求一致的代理出口。
3. Turnstile 按 `YesCaptcha Key -> 本地 Solver URL -> Chromium 回退` 的顺序选择。容器访问宿主机 Solver 时可使用 `http://host.docker.internal:5072`。
4. 注册成功的账号状态为 `grok_registered`，SSO 保存在现有 `session_token` 字段，可从账号详情或运行结果导出。

协议流程参考了 MIT 许可的 [HSJ-BanFan/grok-register-web](https://github.com/HSJ-BanFan/grok-register-web)；xAI 页面或 Server Action 变更时，优先检查任务日志中的动态参数发现错误。

完整配置范围、本地 Solver 接口、凭据落库字段与故障排查见 [`docs/Grok自动注册说明.md`](docs/Grok自动注册说明.md)。

需要复用多套 Grok 参数时，可在“自动化工作流设计师”中编辑内建 Grok 模板，或使用复制按钮创建可独立修改、调度和删除的自定义 Grok 工作流。

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
