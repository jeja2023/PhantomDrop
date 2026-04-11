# PhantomDrop: OpenAI 自动化注册集成最终优化方案

本项目旨在将 OpenAI 账号自动化注册能力原生集成至 PhantomDrop 平台。通过综合借鉴 `CPACM`、`codex-console` 和 `any-auto-register` 的工程化实践，并利用 PhantomDrop 现有的云边缘邮件拦截架构，实现一套高性能、高成功率、低成本的注册管线。

---

## 核心设计哲学 (Optimized Philosophy)

1.  **原生 Rust 引擎 (Native Performance)**: 摒弃 Python 虚拟机，在 Core 层使用 Rust (Axum + Reqwest) 实现纯协议层注册，支持百级并发。
2.  **验证码直连 (Native Verification)**: 利用 PhantomDrop 现有的 Cloudflare Email Worker 链路，注册引擎直接从本地 SQLite 数据库轮询 OTP，彻底消除对外置邮箱 API 的依赖及延迟。
3.  **两阶段状态机 (Dual-Phase State Machine)**: 借鉴 `codex-console` 的最佳实践，将流程分为“注册”与“二次登录”两个阶段，确保 100% 捕获 `access_token` 和 `session_token`。
4.  **架构解耦 (Modular Design)**: 遵循 `any-auto-register` 的插件化思路，将 Sentinel、OAuth、Captcha、Uploader 模块化，确保护持灵活性。

---

## 1. 核心后端架构 (Rust Core)

在 `core/src/openai/` 线下构建全新的自动化套件：

### 1.1 协议基石 (`constants.rs` & `oauth.rs`)
-   **常量定义**: 封装 OpenAI 官方 OAuth 授权 ID (`app_EMoamEEZ73f0CkXaXp7hrann`) 及各 API 端点。
-   **PKCE 流程**: 实现 `Code Verifier` 与 `Code Challenge (S256)` 生成逻辑，处理 Token Exchange (授权码换令牌)。
-   **JWT 解析**: 无感解析 `id_token`，提取 `account_id` 与 `email` 等元数据。

### 1.2 对抗防御 (`sentinel.rs` & `captcha.rs`)
-   **Sentinel PoW**: 实现基于 SHA3-512 的 Proof-of-Work 求解器，应对 OpenAI 的首层拦截。
-   **TLS 指纹模拟**: 针对 `reqwest` 进行指纹优化（JA3/H2），必要时支持 `boring-tls` 链路。
-   **Captcha 接口**: 预留标准化的打码平台 API 接口（YesCaptcha / CapSolver），支持 ArkoseLabs (FunCaptcha) 自动提交。

### 1.3 流程调度 (`register.rs` & `workflow.rs`)
-   **18 步注册状态机**:
    -   1-10 步 (注册期): IP检测 -> 获取 DeviceID -> Sentinel 校验 -> 提交表单 -> 数据库轮询 OTP -> 验证 OTP -> 创建 UserProfile。
    -   11-18 步 (捕获期): 二次登录开启 -> 提交密码 -> 捕获 Session Cookie -> Workspace 选择 -> 流跟随 -> 获取 Access Token。
-   **工作流集成**: 扩展 `WorkflowKind::OpenAIRegister`，支持在 Dashboard 中一键启动。

### 1.4 产物分发 (`uploader.rs`)
-   **CPA 协议**: 支持通过 `multipart/form-data` 将注册结果同步至 Codex 协议平台。
-   **NewAPI/Sub2API**: 支持 JSON 格式的数据回传，支持 `x-api-key` 幂等认证。

---

## 2. 数据与存储 (Persistence)

### 2.1 数据库扩展 (`db.rs`)
扩展 `generated_accounts` 表，完整保留注册产物：
-   `access_token`, `refresh_token`, `session_token`
-   `device_id`, `workspace_id`, `account_id`
-   `source` (标记来源于 register 还是 login)
-   `upload_status` (分发状态追踪)

### 2.2 验证码轮询端点
在 `main.rs` 增加隐形 API `GET /api/otp/poll?email=xxx`，供注册引擎内部调用，实现注册逻辑与邮件接收逻辑的解耦。

---

## 3. 前端界面 (React Paradigm)

### 3.1 极客配置面板
在 `WorkflowTab` 中新增 OpenAI 专有配置区：
-   **并发控制**: 支持设置瞬时并发任务数。
-   **代理配置**: 支持全局或独立住宅代理 URL 设置。
-   **分发开关**: 配置 CPA 或 NewAPI 的 API URL 与密钥。
-   **实时监控**: 利用 SSE 流，像终端一样滚动展示注册状态机每一步的执行日志。

---

## 4. 开放性问题与对策 (Risk Mitigation)

| 风险点 | 状态 | 优化方案 |
| :--- | :--- | :--- |
| **TLS 指纹封锁** | 高风险 | 首期使用 `rustls` 模拟，如被封则迁移至 `boring-tls` 并注入 JA3 签名。 |
| **Arkose 强制验证** | 中风险 | 集成第三方打码平台 API，并在 UI 提供 API Key 填写项。 |
| **SMTP 接收延迟** | 低风险 | PhantomDrop 采用 Cloudflare 直推模式，延迟通常 < 2s，远胜第三方邮箱。 |

---

## 5. 验证与实施路径

1.  **Phase 1**: 在 `core/src/openai/` 实现基础库（Sentinel, OAuth, Constants）。
2.  **Phase 2**: 改造 `parser.rs` 确保精准提取 OpenAI 专有的 6 位 OTP。
3.  **Phase 3**: 实装 `register.rs` 注册引擎及数据库持久化。
4.  **Phase 4**: 前端配置面板上线，开启端到端流程验证。

---

> [!TIP]
> **结论建议**：该方案实现了“中台即引擎”的愿景。通过将原本分散在 Python 脚本中的逻辑下沉至 Rust 核心，PhantomDrop 将进化为具备“账号生产能力”的顶级全栈中台。
