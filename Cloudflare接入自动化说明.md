# Cloudflare 接入自动化说明

## 目标

将 `Cloudflare Email Routing -> Email Worker -> PhantomDrop /ingest` 的接入流程收敛成一个统一脚本：

- 本地 `trycloudflare`
- 公网服务器，无域名
- 公网服务器，有域名

执行入口：

- 首次授权初始化：
  [initialize-cloudflare-automation.ps1](d:/project/PhantomDrop/initialize-cloudflare-automation.ps1)
- 日常自动执行：
  [setup-cloudflare-mail.ps1](d:/project/PhantomDrop/setup-cloudflare-mail.ps1)

## 统一流程

脚本按下面顺序执行：

1. 解析接入模式
2. 规范化公网地址
3. 检查公网 `/health`
4. 如本地后端可达，回填本地 `public_hub_url` 和隧道登记
5. 更新 [wrangler.toml](d:/project/PhantomDrop/network/wrangler.toml)
6. 自动部署 Worker
7. 自动调用 Worker `health` 和 `relay-test`
8. 如具备 Cloudflare API 凭据，自动创建或更新 Email Routing 规则
9. 通过公网 `/ingest` 进行冒烟测试
10. 输出本次结果摘要

## 三种模式

### 1. `local_trycloudflare`

适用于：

- 本地开发
- 没有公网服务器
- 只想临时联调

逻辑：

1. 检查本地 `http://127.0.0.1:4000/health`
2. 自动启动 `cloudflared tunnel --url http://127.0.0.1:4000 --protocol http2 --edge-ip-version 4`
3. 从日志中提取 `https://*.trycloudflare.com`
4. 用这个地址继续后续所有步骤

特点：

- 不需要手填公网地址
- 地址临时，进程退出即失效
- 更适合联调，不适合生产

### 2. `public_ip`

适用于：

- 有公网服务器
- 没有域名
- 后端已经直接暴露在公网 IP 上

输入示例：

- `http://123.45.67.89:4000`
- `https://123.45.67.89`

逻辑：

1. 规范化 IP 形式公网地址
2. 检查该地址 `/health`
3. 更新 Worker 指向该公网地址
4. 自动部署 Worker
5. 如 Cloudflare 邮件域名仍由你控制，仍可自动创建 Email Routing 规则

### 3. `public_domain`

适用于：

- 有公网服务器
- 有域名
- 已为 `hub.example.com` 之类的子域名做好反向代理

输入示例：

- `https://hub.example.com`
- `hub.example.com`

逻辑：

1. 优先补全为 `https://`
2. 检查 `/health`
3. 更新 Worker 指向该域名
4. 自动部署 Worker
5. 自动创建或更新 Email Routing 规则

这是推荐模式。

## 自动化边界

### 脚本可自动完成

- 获取或规范化公网地址
- 回填本地 PhantomDrop 设置
- 更新 Worker 配置
- 部署 Worker
- Worker 到 Hub 的诊断测试
- 公网 `/ingest` 冒烟测试
- Email Routing 规则创建/更新

### 脚本依赖的前置条件

- 本地模式需要：
  - 已安装 `cloudflared`
  - 本地 `core` 已启动
- Worker 部署需要：
  - `npm`
  - `wrangler` 可用
  - 已执行 `wrangler login`
- Email Routing 规则自动创建需要：
  - `CLOUDFLARE_API_TOKEN`
  - `CLOUDFLARE_ZONE_ID`
  - 可选 `CLOUDFLARE_ACCOUNT_ID`
  - 可选 `CLOUDFLARE_ZONE_DOMAIN`

## 推荐使用方式

### 首次授权初始化

首次只需要跑一次，用于固化：

- Cloudflare API Token
- Zone ID
- Account ID
- 域名
- 默认模式
- 默认公网地址

示例：

```powershell
.\initialize-cloudflare-automation.ps1 `
  -DefaultMode public_domain `
  -DefaultPublicUrl "https://hub.example.com" `
  -ZoneDomain "example.com" `
  -CloudflareApiToken "你的 token" `
  -CloudflareZoneId "你的 zone id" `
  -RunWranglerLogin
```

执行后会把配置写入：

- [cloudflare-config.json](d:/project/PhantomDrop/.automation/cloudflare-config.json)

之后主脚本会自动读取，不再要求你重复输入这些值。

### 首次授权后永久自动

完成首次授权后，默认推荐直接运行：

```powershell
.\setup-cloudflare-mail.ps1
```

脚本会自动读取上一次保存的：

- 默认模式
- 默认公网地址
- Hub Secret
- Route Local Part
- Zone Domain
- Cloudflare API Token
- Zone ID
- Account ID

如果是 `public_ip` 或 `public_domain`，后续基本可以做到零参数。

如果是 `local_trycloudflare`，脚本会自动重新拉起 `cloudflared` 并提取新的临时地址。

### 本地 trycloudflare

```powershell
.\启动开发环境.ps1
.\setup-cloudflare-mail.ps1 -Mode local_trycloudflare
```

### 公网服务器，无域名

```powershell
.\setup-cloudflare-mail.ps1 -Mode public_ip -PublicUrl "http://123.45.67.89:4000"
```

### 公网服务器，有域名

```powershell
.\setup-cloudflare-mail.ps1 -Mode public_domain -PublicUrl "https://hub.example.com"
```

## 推荐环境变量

```powershell
$env:CLOUDFLARE_API_TOKEN = "..."
$env:CLOUDFLARE_ZONE_ID = "..."
$env:CLOUDFLARE_ACCOUNT_ID = "..."
$env:CLOUDFLARE_ZONE_DOMAIN = "example.com"
```

设置好这些后，日常只需要传入公网地址即可。

如果已经执行过首次授权初始化，这些环境变量后续可以不再手动设置。

## 自动测试链路

脚本会执行两层测试：

1. **公网 Hub 测试**
   - `GET /health`
   - `POST /ingest`

2. **Worker 测试**
   - `GET /health`
   - `POST /relay-test`

Worker 侧诊断接口由 [network/src/index.ts](d:/project/PhantomDrop/network/src/index.ts) 提供。

## 输出结果

脚本会把最后一次执行摘要写入：

- [cloudflare-mail-last-run.json](d:/project/PhantomDrop/.automation/cloudflare-mail-last-run.json)

其中包含：

- 解析后的模式
- 实际公网地址
- `trycloudflare` 进程 PID
- Worker URL
- Email Routing 规则 ID
- 冒烟测试邮件主题
