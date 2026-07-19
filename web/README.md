# PhantomDrop Web

`web/` 是 PhantomDrop `V0.0.33` 的 React 19 + TypeScript + Vite 管理界面。

## 开发

```powershell
npm ci
npm run dev
```

默认地址为 `http://127.0.0.1:5173`。开发服务器通过 Vite 代理连接本地 Hub；Hub 首次启动需要设置 `ADMIN_USERNAME` 和至少 12 个字符的 `ADMIN_PASSWORD`。

## 登录与会话

- 登录请求发送管理员用户名和密码到 `/auth/login`。
- 成功后仅使用后端签发的 `HttpOnly`、`SameSite=Strict` Cookie。
- 前端不在 `localStorage`、`sessionStorage` 或 JavaScript 状态中保存长期认证密钥。
- `HUB_SECRET` 不属于 Web 登录流程，只用于 Worker 调用 Hub `/ingest`。

## 质量检查

```powershell
npm run lint
npm run build
npm audit --omit=dev --audit-level=high
```

生产构建输出到 `dist/`，由 Rust Hub 或 Docker 单镜像托管。