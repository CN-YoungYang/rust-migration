# 快速开始

## Docker 部署（推荐）

```bash
cd rust-migration

# 1. 生成加密密钥
openssl rand -base64 32

# 2. 创建 .env 文件
cp .env.example .env
# 编辑 .env，填入 TOKEN_ENCRYPTION_KEY

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

## 默认账户

- 用户名：`admin`
- 密码：`admin123456`

**生产环境请立即修改密码！** 可在 `.env` 中通过 `ADMIN_USERNAME` / `ADMIN_PASSWORD` 自定义。

## 验证

```bash
# 健康检查
curl http://localhost:3000/api/health
```

## 技术栈

- **后端：** Rust + Axum 0.7 + SQLite/sqlx + tokio-cron-scheduler + ring + bcrypt
- **前端：** Vue 3 + TypeScript + Vite
- **部署：** Docker 多阶段构建，非 root 用户运行

## 相关文档

- [README.md](../README.md) — 完整文档
- [CHANGELOG.md](../CHANGELOG.md) — 更新日志
