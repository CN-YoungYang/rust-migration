# AI Hub - Rust Migration

自动签到平台的 Rust 重构版本，使用 Axum + SQLite + Vue 3 构建。

## 🎯 新增功能（v2.0）

### 余额查询
- 支持 new-api 和 x666 站点余额查询
- 每个账号旁添加「查询余额」按钮
- 实时更新余额数据

### 记录清理
- 管理员可清理签到记录，保留最新 100 条
- 普通用户只清理自己账号的记录
- 管理员清理所有用户的记录

### 权限控制
**普通用户：**
- 只能查看和操作自己创建的账号
- 只能查看自己账号的签到记录
- 清理时仅删除自己账号的记录

**管理员（ADMIN/SUPER_ADMIN）：**
- 查看所有账号，支持按用户筛选
- 可以筛选：所有用户 / 仅我的 / 指定用户
- 操作所有账号和记录
- 清理时删除所有用户的记录

## 🚀 快速开始

### 构建

```bash
# 1. 构建前端
cd frontend
npm install
npm run build

# 2. 构建后端
cd ..
cargo build --release
```

### 运行

```bash
./target/release/ai-hub-rust
```

默认端口：3000
默认管理员账号：admin / admin123

### Docker 部署

```bash
# 使用 docker-compose
docker-compose up -d

# 或使用 1Panel 配置
docker-compose -f docker-compose.1panel.yml up -d
```

## 📡 API 文档

### 认证
- `POST /api/auth/login` - 登录
- `POST /api/auth/logout` - 登出
- `GET /api/auth/me` - 当前用户信息

### 账号管理
- `GET /api/accounts?userId=xxx` - 获取账号列表（支持用户筛选）
- `POST /api/accounts` - 创建账号
- `GET /api/accounts/:id` - 获取账号详情
- `PUT /api/accounts/:id` - 更新账号
- `DELETE /api/accounts/:id` - 删除账号
- `POST /api/accounts/:id/refresh-balance` - 查询账号余额 ✨

### 签到记录
- `GET /api/checkin-runs` - 获取记录列表
- `POST /api/checkin-runs` - 手动执行签到
- `DELETE /api/checkin-runs/cleanup` - 清理记录 ✨

### 设置
- `GET /api/settings` - 获取设置
- `PUT /api/settings` - 更新设置

### 用户管理（仅管理员）
- `GET /api/users` - 用户列表
- `POST /api/users` - 创建用户
- `PUT /api/users/:id` - 更新用户
- `DELETE /api/users/:id` - 删除用户

## 🗄️ 数据库

SQLite 数据库，位置：`./data/app.db`

### 表结构

**AppUser** - 用户表
- `ADMIN` - 管理员
- `SUPER_ADMIN` - 超级管理员
- `USER` - 普通用户

**CheckinAccount** - 签到账号表
- `ownerId` - 账号所有者（关联 AppUser.id）
- 支持 new-api / anyrouter / x666 站点类型

**CheckinRun** - 签到记录表
**CheckinSetting** - 全局设置表

## 🔒 安全

- 敏感字段（access_token, cookie）使用 AES-256-GCM 加密存储
- 密码使用 bcrypt 加密
- JWT token 认证
- 权限控制基于用户角色和资源所有权

## 📦 技术栈

- **后端**: Rust + Axum + SQLx + SQLite
- **前端**: Vue 3 + TypeScript + Vite
- **加密**: AES-256-GCM
- **认证**: JWT

## 📝 环境变量

```bash
PORT=3000                          # 服务端口
DATABASE_URL=sqlite:./data/app.db  # 数据库路径
ENCRYPTION_KEY=<32字节hex>         # 加密密钥（自动生成）
JWT_SECRET=<随机字符串>            # JWT密钥（自动生成）
```

## 🔄 迁移说明

从 TypeScript 版本迁移：

1. 数据库兼容 - 使用相同的 SQLite schema
2. 需要执行迁移脚本添加 `ownerId` 字段
3. 前端 API 保持兼容

## 📄 License

MIT