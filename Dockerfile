# ==========================================
# Phase 1: Build Frontend (Web)
# ==========================================
FROM node:22.17.0-bookworm-slim AS web-builder

WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci

COPY web/ ./
RUN npm run build
# ==========================================
# Phase 2: Prepare Worker Runtime
# ==========================================
FROM node:22.17.0-bookworm-slim AS network-builder

WORKDIR /build/network
COPY network/package.json network/package-lock.json ./
RUN npm ci
COPY network/ ./

# ==========================================
# Phase 3: Build Backend (Core)
# ==========================================
FROM rust:1.88.0-bookworm AS core-builder

WORKDIR /build/core
# 安装构建 boring-sys 所需的系统依赖 (cmake, golang)
RUN apt-get update && apt-get install -y --no-install-recommends cmake golang clang libclang-dev && rm -rf /var/lib/apt/lists/*
# 缓存依赖层：先只拷贝 Cargo.toml 和 Cargo.lock 构建依赖
COPY core/Cargo.toml core/Cargo.lock ./
# 创建一个空的 main.rs 来欺骗 cargo 编译所有的依赖包
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && CARGO_BUILD_JOBS=1 cargo build --release --locked \
    && rm -rf src

# 拷贝真实的业务代码和资源
COPY core/src ./src
COPY core/console ./console
COPY core/migrations ./migrations

# 更新 main.rs 的时间戳，强制 Cargo 重新编译我们的业务代码，而不是使用上面的空缓存
RUN touch src/main.rs && CARGO_BUILD_JOBS=1 cargo build --release --locked

# ==========================================
# Phase 4: Final Runtime Image
# ==========================================
FROM debian:12.11-slim

# 1. 设置非交互模式和基础依赖
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    wget \
    gnupg \
    git \
    && rm -rf /var/lib/apt/lists/*

# 2. 安装 PowerShell (pwsh) 及 Chromium
RUN wget -q "https://packages.microsoft.com/config/debian/12/packages-microsoft-prod.deb" \
    && dpkg -i packages-microsoft-prod.deb \
    && rm packages-microsoft-prod.deb \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
    powershell \
    chromium \
    chromium-sandbox \
    xvfb \
    xauth \
    libnss3 \
    libxss1 \
    libasound2 \
    libgbm1 \
    fonts-ipafont-gothic fonts-wqy-zenhei fonts-thai-tlwg fonts-kacst fonts-freefont-ttf \
    && rm -rf /var/lib/apt/lists/*

# 设置环境变量，指向 Chromium 路径
ENV CHROME_BIN=/usr/bin/chromium
ENV CHROME_PATH=/usr/lib/chromium/
ENV DISPLAY=:99

# 3. 安装 Node.js (用于 wrangler)
COPY --from=network-builder /usr/local/bin/node /usr/local/bin/node
COPY --from=network-builder /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx
# 4. 安装 cloudflared
ARG CLOUDFLARED_VERSION=2026.7.2
ARG CLOUDFLARED_SHA256=88195157a136199a86977c122a22084dae6907480bbe3640222b7b55834afc3a
RUN curl -fL --retry 3 --output /tmp/cloudflared.deb "https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}/cloudflared-linux-amd64.deb" \
    && echo "${CLOUDFLARED_SHA256}  /tmp/cloudflared.deb" | sha256sum --check --strict \
    && dpkg -i /tmp/cloudflared.deb \
    && rm /tmp/cloudflared.deb
WORKDIR /app

# 5. 复制后端二进制
COPY --from=core-builder /build/core/target/release/core /app/phantom-core

# 6. 复制前端产物 (存放在 web 目录供后端整合托管)
COPY --from=web-builder /build/web/dist /app/web

# 7. 复制自动化脚本和必要工具
COPY ./setup-cloudflare-mail.ps1 /app/
COPY ./initialize-cloudflare-automation.ps1 /app/
COPY --from=network-builder /build/network /app/network
COPY ./tools /app/tools

# 8. 环境配置
ENV APP_ENV=production
ENV PHANTOM_DB_URL=sqlite:///app/data/phantom_core.db?mode=rwc
ENV WEB_DIST=/app/web
ENV BIND_ADDR=0.0.0.0
ENV PORT=9010
ENV ENABLE_DEBUG_ASSETS=false
ENV WRITE_CODEX_AUTH_FILE=false
ENV ADMIN_USERNAME=admin
ENV PHANTOM_GATEWAY_KEYS=
LABEL org.opencontainers.image.title="PhantomDrop" \
      org.opencontainers.image.version="0.0.34"
# 确保在容器内通过 pwsh 运行
ENV SHELL=/usr/bin/pwsh

# 暴露端口 (统一入口)
EXPOSE 9010
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD curl -fsS http://127.0.0.1:9010/health || exit 1

# 建立数据持久化目录
RUN useradd --system --uid 10001 --create-home phantom && mkdir -p /app/data /app/.automation && chown -R phantom:phantom /app
USER phantom

# 启动 (使用 xvfb-run 虚拟显示环境)
CMD ["xvfb-run", "--server-args=-screen 0 1920x1080x24", "/app/phantom-core"]
