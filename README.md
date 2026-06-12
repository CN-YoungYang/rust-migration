# AI Hub - Rust Migration

自动签到平台的 Rust 重构版本，使用 Axum + SQLite + Vue 3 构建。

## 🎯 新增功能（v2.0）

### 用户管理
- 完整的用户CRUD操作
- 角色权限控制（USER/ADMIN/SUPER_ADMIN）
- ADMIN只能管理USER，SUPER_ADMIN可管理所有（除其他SUPER_ADMIN）
- SUPER_ADMIN账户受保护，无法删除或降级

### 签到账户管理
- 支持 new-api 和 x666 站点
- Token/Cookie 认证方式
- 实时余额查询
- 账户启用/禁用

### 签到历史
- 查看所有签到记录
- 手动执行签到
- 批量清理历史

### 全局设置
- 自动签到调度配置
- Cron表达式支持
- 重试和超时设置

## 🚀 快速开始

### 环境要求
- Rust 1.75+
- Node.js 18+
- SQLite 3

### 开发环境

```bash
# 1. 克隆项目
git clone <repo-url>
cd rust-migration

# 2. 配置环境变量（已包含默认.env）
# 可选：修改 .env 文件自定义配置

# 3. 启动后端
cargo run

# 4. 启动前端（新终端）
cd frontend
npm install
npm run dev

# 5. 访问应用
# http://localhost:5173
```

### 生产部署

```bash
# 1. 构建前端
cd frontend
npm install
npm run build

# 2. 构建后端
cd ..
cargo build --release

# 3. 运行
./target/release/ai-hub-rust

# 或使用 Docker（容器端口 8080）
docker build -t ai-hub .
docker run -p 8080:8080 -v $(pwd)/data:/app/data ai-hub
```

## 🔐 默认账户

**超级管理员：**
- 用户名：`admin`
- 密码：`admin123456`

**⚠️ 生产环境请立即修改密码！**

## 📁 项目结构

```
rust-migration/
├── src/                  # Rust 后端源码
│   ├── routes/          # API 路由
│   ├── services/        # 业务逻辑（签到、调度器）
│   ├── models.rs        # 数据模型
│   ├── db.rs            # 数据库操作
│   └── main.rs          # 入口文件
├── frontend/            # Vue 3 前端
│   ├── src/
│   │   ├── components/  # UI 组件
│   │   ├── config.ts    # API 配置
│   │   └── App.vue      # 主应用
│   ├── .env.development # 开发环境配置
│   └── .env.production  # 生产环境配置
├── migrations/          # 数据库迁移
├── .env                 # 环境变量（已配置）
└── Dockerfile           # Docker 构建文件
```

## 🔌 API 端点

### 认证
- `POST /api/auth/login` - 登录
- `GET /api/auth/me` - 获取当前用户
- `POST /api/auth/logout` - 登出

### 用户管理（需要管理员权限）
- `GET /api/admin/users` - 用户列表
- `POST /api/admin/users` - 创建用户
- `PUT /api/admin/users/:id` - 更新用户
- `DELETE /api/admin/users/:id` - 删除用户

### 签到账户
- `GET /api/accounts` - 账户列表
- `POST /api/accounts` - 创建账户
- `PUT /api/accounts/:id` - 更新账户
- `DELETE /api/accounts/:id` - 删除账户
- `POST /api/accounts/:id/refresh-balance` - 刷新余额

### 签到记录
- `GET /api/checkin-runs` - 签到历史
- `POST /api/checkin-runs` - 执行签到
- `DELETE /api/checkin-runs/cleanup` - 清理记录

### 设置
- `GET /api/settings` - 获取设置
- `PUT /api/settings` - 更新设置

## ⚙️ 配置说明

### 后端 (.env)
```env
DATABASE_URL=sqlite:./data/ai-hub.db
TOKEN_ENCRYPTION_KEY=<base64编码的32字节密钥>
# 生成方式: openssl rand -base64 32
ADMIN_USERNAME=admin
ADMIN_PASSWORD=<至少8位强密码>
RUST_LOG=info
TZ=Asia/Shanghai
CORS_ALLOWED_ORIGINS=http://localhost:5173
SESSION_TTL_HOURS=24
```

### 前端环境变量
- 开发：`.env.development` → `http://localhost:8080/api`
- 生产：`.env.production` → `/api` （反向代理）

## 🛡️ 权限矩阵

| 操作 | USER | ADMIN | SUPER_ADMIN |
|------|------|-------|-------------|
| 查看自己的账户 | ✅ | ✅ | ✅ |
| 查看所有账户 | ❌ | ✅ | ✅ |
| 管理USER | ❌ | ✅ | ✅ |
| 管理ADMIN | ❌ | ❌ | ✅ |
| 管理SUPER_ADMIN | ❌ | ❌ | ❌ |

## 📝 技术栈

**后端：**
- Rust + Axum（Web框架）
- SQLite + sqlx（数据库）
- tokio-cron-scheduler（定时任务）
- JWT认证

**前端：**
- Vue 3 + TypeScript
- Vite（构建工具）
- 原生CSS

## 🔄 从旧版本迁移

如果你从 Node.js 版本迁移：
1. 数据库结构兼容
2. 需要重新生成JWT token
3. 密码需要重新加密

## 📄 许可证

MIT


