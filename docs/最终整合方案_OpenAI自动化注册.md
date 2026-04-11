# PhantomDrop: OpenAI 原生自动化注册最终集成方案

本方案旨在将 OpenAI 账号自动化注册能力作为“一级公民”原生集成至 PhantomDrop 平台。通过 Rust 协议栈实现，性能比传统浏览器自动化提升 10 倍以上，且具备极强的工程可维护性。

---

## 1. 架构概览 (Architecture)

采用 **"Backend-as-Engine"** 模式，将注册逻辑下沉至 `core` 层。

- **引擎层 (Rust)**: 负责 PoW 求解、OAuth 协议握手、Cookie 维护。
- **数据层 (SQLite)**: 负责存储令牌（Access/Refresh/Session）及设备指纹。
- **通信层 (Cloudflare)**: 负责邮件拦截，作为注册引擎的“虚拟收件箱”。
- **展示层 (React)**: 负责配置分发策略及实时状态监控。

---

## 2. 后端核心模块设计 (`core/src/openai/`)

### 2.1 协议基石 (`constants.rs`, `oauth.rs`)
- **PKCE 实现**: 手写基于 `S256` 的 `code_challenge` 生成逻辑。
- **常量定义**: 封装 OpenAI 的官方 ClientID (`app_EMoamEEZ73f0CkXaXp7hrann`)。
- **JWT 无感解析**: 引入 `jsonwebtoken` 或手动 Base64 解析 `id_token`，提取 `account_id`（OpenAI 用户唯一 ID）。

### 2.2 抗逆盾模块 (`sentinel.rs`, `captcha.rs`)
- **SHA3 PoW**: 实现基于 `sha3` 库的高性能 Proof-of-Work 求解器。
- **Sentinel 状态机**: 模拟 `sentinel.openai.com/backend-api/sentinel/req` 的完整握手流程，获取关键的 `sentinel_token`。
- **打码平台适配**: 标准化接口，首期支持 YesCaptcha API 用于处理 Arkose Labs (FunCaptcha) 的人机验证。

### 2.3 注册状态机 (`register.rs`)
借鉴 `codex-console` 的**两阶段模式**：
1.  **Phase A (Registration)**:
    - 检查 IP 纯净度 -> 获取 DeviceID -> Sentinel 校验 -> 提交注册邮箱 -> 设置密码 -> 轮询本地 DB 获取 OTP -> 验证 OTP -> 创建 UserProfile。
2.  **Phase B (Post-Login & Capture)**:
    - 以登录模式重启会话 -> 提交密码 -> 捕获 `oai-client-auth-session` -> 选择 Workspace -> 流重定向跟随 -> 获取最后的 `access_token`。

---

## 3. 基础设施升级

### 3.1 数据库字段扩展 (`db.rs`)
在 `generated_accounts` 表中增加以下关键字段：
- `access_token`, `refresh_token`, `session_token` (注册核心产物)
- `device_id`, `workspace_id` (指纹环境记录)
- `upload_status` (追溯账号是否已分发至 CPA/NewAPI)

### 3.2 邮件解析增强 (`parser.rs`)
- **精准正则**: 使用 `(?<!\d)(\d{6})(?!\d)` 匹配 6 位验证码。
- **白名单过滤**: 仅处理来自 `openai.com` 的注册验证邮件，防止干扰。

### 3.3 内部轮询 API (`main.rs`)
- 新增 `GET /api/internal/otp/poll?email=...`，供注册引擎在注册过程中实时查询邮件表，实现完全闭环。

---

## 4. 后台分发系统 (`uploader.rs`)

支持将注册成功的账号实时推送至下游：
- **CPA 协议**: `multipart/form-data` 格式，对齐 `CPACM` 规范。
- **NewAPI/Sub2API**: `application/json` 格式，支持 `x-api-key`鉴权。

---

## 5. 前端 UI 极客面板

### 5.1 工作流配置
- **任务面板**: 支持设置“批量注册”的目标数量、并发数、使用的代理 URL 池。
- **分发开关**: 勾选“上传至 CPA”或“上传至 NewAPI”，并填写对应的 API Key。

### 5.2 实时流监控
- 利用现有的 SSE 事件流，将注册状态机的每一步（如 `[Step 4] PoW Solved in 1.2s`, `[Step 8] OTP Received: 123456`）以终端滚屏形式展现。

---

## 6. 风险评估与对策

| 风险点 | 等级 | 对策 |
| :--- | :--- | :--- |
| **TLS 指纹风控** | 高 | 初期使用标准 `rustls`，如被封则迁移至 `boring-tls` 手动注入 JA3 签名。 |
| **Arkose 强制验证** | 中 | 集成第三方打码平台 API，按需消耗 API 余额。 |
| **手机号验证 (SMS)** | 高 | 首期针对无需手机号的区域（或免手机号策略）进行优化，后续可扩展虚拟号接码 API。 |

---

## 7. 实施路线图 (Milestones)

1.  **阶段一**: 搭建 `core/src/openai/` 基础结构，实现 PoW 和 OAuth 逻辑。
2.  **阶段二**: 优化邮件解析器，确保 100% 捕获 OpenAI OTP。
3.  **阶段三**: 实装两阶段注册引擎，完成“账号生产 -> 本地存储”闭环。
4.  **阶段四**: 开发前端配置面板与实时流显示，上线批量注册功能。
