# OpenAI 自动化注册功能 - 实施计划

## 当前状态分析

当前 `openai_register_flow` 是一个**纯模拟流程**（用 `sleep` 模拟延迟，硬编码假地址和密码）。需要按照最终整合方案进行实质性升级。

## 本次改动范围

鉴于 OpenAI 的完整协议栈（PKCE、Sentinel PoW、Arkose 打码等）需要大量逆向工作且高度依赖实时的 API 认知，本次实施聚焦于**架构搭建 + 基础设施完善 + 前端增强**，为后续接入真实协议打下坚实基座。

### 后端改动

1. **数据库扩展** (`db.rs`)
   - `generated_accounts` 表增加 `access_token`、`refresh_token`、`session_token`、`device_id`、`workspace_id`、`upload_status` 字段
   - 新增 `update_generated_account_tokens()` 方法
   - 新增 `poll_otp_by_email()` 方法（内部 OTP 轮询）
   - 新增 `list_all_generated_accounts()` 方法（全局账号列表）

2. **邮件解析器增强** (`parser.rs`)
   - 增加 OpenAI OTP 专用正则：精准匹配 6 位纯数字 `(?<!\d)(\d{6})(?!\d)`
   - 增加 OpenAI 发件人白名单过滤能力

3. **OpenAI 协议模块** (`core/src/openai/`)
   - `constants.rs` — 常量定义（ClientID、端点 URL 等）
   - `oauth.rs` — PKCE 工具函数（code_verifier/code_challenge 生成）
   - `sentinel.rs` — Sentinel/PoW 解算接口抽象
   - `register.rs` — 两阶段注册状态机框架
   - `mod.rs` — 模块导出整合

4. **工作流引擎升级** (`workflow.rs`)
   - `openai_register_flow` 接入真实模块调用链
   - 增加 `batch_size` 参数支持

5. **新增内部 API** (`main.rs`)
   - `GET /api/otp/poll?email=xxx` — 供注册引擎内部轮询 OTP
   - `GET /api/accounts` — 全局账号列表
   - `POST /api/accounts/:id/tokens` — 更新 Token

### 前端改动

6. **前端类型** (`types.ts`)
   - `GeneratedAccountRecord` 增加 token 相关字段

7. **注册视图增强** (`RegistrationView.tsx`)
   - 增加批量数量配置
   - 增加打码平台 API Key 配置
   - 增加账号分发开关（CPA/NewAPI）
   - 优化实时日志流自动滚动

## 实施顺序

1. 后端：DB 扩展 → 解析器增强 → OpenAI 模块 → 工作流优化 → API 路由
2. 前端：类型 → 视图组件
