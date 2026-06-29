# 快速开始

## Docker 部署（推荐）

```bash
cd rust-migration

# 1. 生成加密密钥
openssl rand -base64 32

# 2. 创建 .env 文件
cp .env.example .env
# 编辑 .env，填入 TOKEN_ENCRYPTION_KEY 和首次启动用的 ADMIN_PASSWORD

# 3. 启动服务
docker compose -f docker-compose.hub.yml up --build -d

# 4. 访问
# http://localhost:3000
```

## 1C1G 服务器一键部署

```bash
bash deploy-1c1g.sh
```

## 本地开发

```bash
# 后端
cp .env.example .env
cargo run

# 前端（新终端）
cd frontend && npm install && npm run dev
# http://localhost:5173
```

## 初始管理员

首次启动时会根据 `.env` 创建管理员：

- `ADMIN_USERNAME`：默认 `admin`
- `ADMIN_PASSWORD`：首次启动必填，至少 8 位

生产环境请使用强密码。管理员创建后，密码会以 bcrypt 哈希存储；首次登录后建议修改密码并移除部署环境中的 `ADMIN_PASSWORD`。

## 验证

```bash
# 健康检查
curl http://localhost:3000/api/health
```

浏览器打开 `http://localhost:3000`，使用 `.env` 中的管理员账号登录。

## 技术栈

- **后端：** Rust + Axum 0.7 + SQLite/sqlx + tokio-cron-scheduler + ring + bcrypt
- **前端：** Vue 3 + TypeScript + Vite
- **部署：** Docker 多阶段构建，非 root 用户运行

## 相关文档

- [README.md](../README.md) — 完整文档
- [CHANGELOG.md](../CHANGELOG.md) — 更新日志
- [OPERATIONS.md](OPERATIONS.md) — 操作指南
- [ADMIN-FEATURES.md](ADMIN-FEATURES.md) — 管理员功能
