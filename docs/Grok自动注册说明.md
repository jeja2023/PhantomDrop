# Grok 自动注册说明

本文适用于 PhantomDrop `V0.0.38`，说明内建 `grok_register_default` 工作流的启用条件、配置边界和运行结果。注册协议参考 MIT 许可项目 [HSJ-BanFan/grok-register-web](https://github.com/HSJ-BanFan/grok-register-web)，并按 PhantomDrop 的邮件中枢、工作流和账号资产模型完成集成。

## 启用条件

1. 在系统设置中配置公网可收信的 `account_domain`；未配置时可回退到 `cloudflare_zone_domain`。`.local`、IP 地址和带协议的 URL 会被拒绝。
2. 确认 Cloudflare Email Routing 已把该域名的邮件转发到 Hub，且目标随机邮箱无需预先创建。
3. Hub 进程必须配置与 Worker 一致的 `HUB_SECRET`，否则任务会在访问 xAI 前停止。
4. 确认 Hub 到 xAI、所选 Solver 和代理出口的网络连通性。
5. 使用 Chromium 回退时，运行环境需要可启动 Chrome/Chromium；无头模式无法人工介入挑战。Docker 部署保持 `PHANTOM_CHROME_SANDBOX=false`，不要通过给容器增加 `SYS_ADMIN` 权限解决 namespace 错误。

## 执行流程

1. 为每个任务生成随机邮箱、本地密码和独立 HTTP Cookie 会话。
2. 从 xAI 注册页及其 Next.js 静态资源动态发现 Turnstile Site Key 与 Server Action ID；协议访问被拦截时使用 Chromium 重新发现。
3. 请求 xAI 发送邮箱验证码，并在 PhantomDrop 邮件库中轮询本次请求之后到达的目标邮件。
4. 优先从主题提取 `ABC-123`，再检查纯文本和 HTML，提交前规范化为 `ABC123`。
5. 按当前配置选择 Turnstile 处理器，验证邮箱验证码后提交注册资料。
6. 仅跟随受信任的 xAI/Grok 认证地址，提取 `sso` Cookie，并尝试完成服务条款和生日初始化。
7. 将账号写入 `generated_accounts`；任务运行日志和摘要通过现有 SSE 实时推送。

注册后初始化属于尽力执行：初始化请求失败会写入警告日志，但已成功取得的账号和 SSO 仍会落库。

## 工作流参数

| 参数 | 默认值 | 约束与说明 |
| --- | --- | --- |
| `account_domain` | 系统设置值 | 必须是公网可收信域名；解析顺序为工作流参数、系统 `account_domain`、`cloudflare_zone_domain`，空字符串不会阻断回退。 |
| `batch_size` | `1` | 单次创建数量，范围 `1..=50`。 |
| `concurrency` | `1` | 同批并发数，范围 `1..=10`，且不会超过批次大小。 |
| `proxy_url` | 空 | 注册 HTTP 会话使用的代理；YesCaptcha 任务会同步代理信息。 |
| `captcha_key` | 空 | 配置后选择 YesCaptcha，优先级高于本地 Solver。 |
| `turnstile_solver_url` | 空 | 本地 Solver 基础 URL，只接受不含内嵌凭据的 HTTP/HTTPS 地址。 |
| `headless` | `true` | 仅影响 Chromium Turnstile 回退；设为 `false` 可显示浏览器窗口。 |
| `registration_timeout_secs` | `180` | 单任务网络、验证码和 Solver 等待基线，范围 `60..=600` 秒。 |

在“自动化中心 -> Grok 注册中心”中保存这些参数，然后启动 `grok_register_default`。Grok 与 OpenAI 使用互斥的一级页面入口，运行记录仍由底部统一监控区展示；任务可从运行列表终止，并在下一次取消检查时退出。

## 运行前就绪检查

Grok 注册面板会在保存和启动前调用 `GET /api/workflows/grok/readiness`，检查收信域名、`HUB_SECRET`、公网 Hub、Cloudflare Zone、近 30 天邮件、代理与 Solver/Chromium。`fail` 项会阻止启动，`warn` 项允许启动但会降低端到端成功把握；面板同时显示实际采用的域名来源和 Solver 模式。

这项检查验证的是 PhantomDrop 本地配置和已知邮件链路证据，不会为了探测而向 xAI 发起注册，也不会修改 Cloudflare 配置。第三方页面与风控仍可能在运行时变化，因此“已就绪”不等于保证每个外部注册请求成功。

## 在工作流设计师中编排

“Grok 注册中心”适合保存当前配置并立即运行；需要维护多套配置时，使用“自动化工作流设计师”中的“Grok 自动化注册”卡片。

1. 点击“编排参数”可修改标题、摘要、批次数量、收信域名、并发数、代理地址、人机验证方案、浏览器模式和单账号超时时间。
2. 内建模板可以修改参数但不能删除。
3. 点击复制图标会生成新的 `grok_register_<时间戳>` 自定义工作流，并自动打开参数编辑器。
4. 自定义副本使用独立 ID，可以分别调度和删除，不会覆盖内建 `grok_register_default`。

所有 Grok 模板继续使用同一个后端工作流类型和参数校验。复制操作只创建工作流定义，不会立即发起外部注册。

## Turnstile 选择规则

处理器按以下顺序选择，并在单个处理器失败时继续回退：

1. `captcha_key` 非空时优先尝试 YesCaptcha。
2. `turnstile_solver_url` 非空时尝试本地 Solver。
3. 前述方式未配置或失败时使用 Chromium 回退。

本地 Solver 需要兼容以下只读接口：

- `GET /turnstile?url=<注册页>&sitekey=<站点密钥>&proxy=<可选代理>`，返回 `taskId`。
- `GET /result?id=<taskId>`，完成时返回 `solution.token`。

容器访问宿主机 Solver 时通常使用 `http://host.docker.internal:5072`。Chromium 回退不支持带用户名/密码的认证代理；此场景应配置 YesCaptcha 或本地 Solver。

## 结果与凭据

成功记录写入现有账号库：

- `status`：`grok_registered`
- `account_type`：`grok_free`
- `session_token`：Grok `sso` Cookie 值
- `email` / `password` / `proxy_url`：本次注册使用的账号和出口信息

SSO 和 Solver Key 都属于敏感凭据。不要把账号详情、数据库文件或工作流配置导出到不受信任的位置；任务日志不会主动输出完整 SSO 或 Solver Key。

## 故障排查

- **动态参数发现失败**：xAI 页面结构或 Server Action 已变化。先检查注册页是否被代理、地区或挑战页替换，再根据运行日志更新发现逻辑。
- **等待验证邮件超时**：确认 `account_domain`、Email Routing、Worker `HUB_SECRET` 和 `/ingest` 状态；邮件时间必须晚于本次验证码请求。
- **Solver 一直处理中**：确认其返回 `taskId` 和 `solution.token`，以及注册请求与验证码任务使用相同代理出口。
- **Chromium 无法启动**：检查容器/主机中的 Chrome、共享内存和执行权限。出现 `Failed to move to new namespace` 时确认 Docker 中 `PHANTOM_CHROME_SANDBOX=false`；带认证代理请改用外部 Solver。
- **注册页返回 HTTP 403**：说明 xAI/Cloudflare 拒绝当前出口；配置可用的 Grok 代理，并确保协议请求、浏览器和 Turnstile 任务使用一致出口。关闭 Chromium sandbox 不能解决网络风控。
- **部分批次失败**：工作流会继续完成其他任务，并在摘要中分别统计成功与失败；从对应邮箱前缀的步骤日志定位原因。

第三方注册页面和风控策略可能随时变化。升级或修改协议后应先用隔离数据库、小批次和受控邮箱域名验证，不要在测试中复用生产账号资产。
