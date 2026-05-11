# 本地验证指南

在普通 PowerShell 终端中运行以下命令，验证后端、前端和 Worker 的基础质量门禁。

## Rust 后端

```powershell
cd D:\project\PhantomDrop\core
cargo test --target-dir target-local
cargo test openai::oauth --target-dir target-local -- --nocapture
```

如果遇到 `拒绝访问 (os error 5)`，通常是当前执行环境无法写入 Cargo target 目录。请换到普通用户终端，或清理被占用的 target 目录后重试。

## Web 前端

```powershell
cd D:\project\PhantomDrop\web
npm run build
```

如果 Vite/Tailwind 原生模块报 `spawn EPERM`，通常是沙箱阻止了 Node 子进程或原生模块加载。请在普通 PowerShell 终端中运行。

## Cloudflare Worker

```powershell
cd D:\project\PhantomDrop\network
npm run typecheck
```

`npm run dry-run` 会调用 Wrangler，可能与 Cloudflare 通信并传输打包元数据；只在明确允许外联验证时运行。
