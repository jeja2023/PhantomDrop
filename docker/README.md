# Docker 部署说明

## 目录说明

- `Dockerfile.core`：构建并运行 Rust 后端
- `Dockerfile.web`：构建前端并交给 Nginx 提供静态服务
- `nginx/default.conf`：前端静态资源与后端接口反向代理配置
- `docker-compose.yml`：本地或服务器编排入口

## 启动方式

在项目根目录执行：

```bash
docker compose -f docker/docker-compose.yml up --build -d
```

启动后：

- 前端地址：`http://服务器地址:8080`
- 后端地址：`http://服务器地址:4000`

## 数据持久化

后端使用 `phantomdrop_data` 卷保存数据库文件，容器重建后数据会保留。

## 可选环境变量

- `HUB_SECRET`
- `PHANTOM_DB_URL`

例如：

```bash
HUB_SECRET=your_secret docker compose -f docker/docker-compose.yml up --build -d
```
