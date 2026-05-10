# ==========================================
# Phase 1: Build Frontend (Web)
# ==========================================
FROM node:22-bookworm-slim AS web-builder

WORKDIR /build/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --legacy-peer-deps

COPY web/ ./
RUN npm run build

# ==========================================
# Phase 2: Build Backend (Core)
# ==========================================
FROM rust:1.88-bookworm AS core-builder

WORKDIR /build/core
# 安装构建 boring-sys 所需的系统依赖 (cmake, golang)
RUN apt-get update && apt-get install -y --no-install-recommends cmake golang clang libclang-dev && rm -rf /var/lib/apt/lists/*
# 缓存依赖层 (可选优化，此处先简单处理)
COPY core/Cargo.toml core/Cargo.lock ./
COPY core/src ./src
COPY core/console ./console
COPY core/migrations ./migrations

RUN cargo build --release

# ==========================================
# Phase 3: Final Runtime Image
# ==========================================
FROM debian:bookworm-slim

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
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

# 4. 安装 cloudflared
RUN curl -L --output /tmp/cloudflared.deb https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.deb \
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
COPY ./network /app/network
COPY ./tools /app/tools
COPY ./.automation /app/.automation

# 8. 环境配置
ENV APP_ENV=development
ENV PHANTOM_DB_URL=sqlite:///app/data/phantom_core.db?mode=rwc
ENV WEB_DIST=/app/web
ENV BIND_ADDR=0.0.0.0
ENV PORT=9010
ENV ENABLE_DEBUG_ASSETS=false
ENV WRITE_CODEX_AUTH_FILE=false
# 确保在容器内通过 pwsh 运行
ENV SHELL=/usr/bin/pwsh

# 暴露端口 (统一入口)
EXPOSE 9010

# 建立数据持久化目录
RUN mkdir -p /app/data

# 启动 (使用 xvfb-run 虚拟显示环境)
CMD ["xvfb-run", "--server-args=-screen 0 1920x1080x24", "/app/phantom-core"]
