# AI Hub - Rust 自动签到平台

基于 Rust + Axum + SQLite + Vue 3 的自动签到管理平台，针对 1C1G 低配服务器优化。

## 功能

- **多站点签到** — 支持 new-api、anyrouter、x666 三种站点类型
- **用户管理** — USER / ADMIN / SUPER_ADMIN 三级角色，完整 CRUD
- **所有权隔离** — 普通用户只能查看和操作自己的账户，管理员可管理所有
- **自动调度** — 可配置签到窗口（本地时间）、重试策略、每日尝试上限
- **余额查询** — 支持 new-api、anyrouter 和 x666 站点的实时余额查询
- **签到反馈** — 签到成功后显示本次获得额度（quota + 美元换算，500000 quota = 1 USD）
- **余额联动** — 签到成功 / 已签到后自动刷新账户余额（失败不阻塞签到）
- **反爬处理** — arrouter 自动识别并求解 acw_sc__v2 反爬挑战
- **记录管理** — 签到记录查看、手动执行、批量清理
- **安全加固** — 登录频率限制、会话管理、AES-256-GCM 加密存储、时序攻击防护

## 快速开始

### Docker 部署（推荐）

> 采用多阶段构建：Stage 1 使用 `node:22-slim` 编译前端，Stage 2 使用 `rust:1.86-slim` 编译后端并打包运行时镜像。**无需在宿主机预构建前端**，镜像构建时会自动产出最新 `public/`。

```bash
# 1. 生成加密密钥
openssl rand -base64 32

# 2. 配置环境变量
cp .env.example .env
# 编辑 .env，填入 TOKEN_ENCRYPTION_KEY

# 3. 构建并启动
docker compose -f docker-compose.hub.yml up --build -d

# 4. 访问
# http://localhost:3000
```

### 本地开发

```bash
# 后端
cp .env.example .env  # 编辑配置
cargo run

# 前端（新终端）
cd frontend
npm install
npm run dev
# http://localhost:5173
```

### 生产构建

```bash
cd frontend && npm install && npm run build && cd ..
cargo build --release
./target/release/ai-hub-rust
```

## 默认账户

- 用户名：`admin`
- 密码：`admin123456`

**生产环境请立即修改密码！**

## 项目结构

```
rust-migration/
├── src/
│   ├── main.rs              # 入口、路由注册、CORS、中间件
│   ├── models.rs            # 数据模型 + serde/sqlx 映射
│   ├── db.rs                # 数据库操作（SQLite）
│   ├── error.rs             # 统一错误处理
│   ├── crypto.rs            # AES-256-GCM 加密 + bcrypt 密码
│   ├── auth_middleware.rs   # 会话管理 + 鉴权
│   ├── routes/
│   │   ├── auth.rs          # 登录/登出/me（含频率限制）
│   │   ├── accounts.rs      # 签到账户 CRUD + 余额刷新
│   │   ├── admin.rs         # 用户管理（仅管理员）
│   │   ├── settings.rs      # 全局设置（仅管理员）
│   │   └── checkin_runs.rs  # 签到记录 + 手动执行 + 清理
│   └── services/
│       ├── scheduler.rs     # Cron 调度器（并发控制）
│       └── checkin/
│           ├── runner.rs    # 签到执行器
│           └── providers/   # 站点适配器
│               ├── new_api.rs
│               ├── anyrouter.rs
│               └── x666.rs
├── frontend/
│   └── src/
│       ├── App.vue          # 主应用 + 路由
│       ├── components/      # 4 个面板组件
│       └── utils/
│           ├── api.ts       # 请求封装 + 错误解析
│           └── toast.ts     # Toast + 确认弹窗
├── migrations/
│   └── 20260611_init.sql    # 建表 + 索引 + 默认数据
├── Dockerfile               # 多阶段构建（非 root 用户）
├── docker-compose.hub.yml   # 生产部署配置
└── deploy-1c1g.sh           # 1C1G 服务器一键部署脚本
```

## API 端点

### 认证
| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/auth/login` | 登录（含频率限制） |
| GET | `/api/auth/me` | 获取当前用户 |
| POST | `/api/auth/logout` | 登出 |

### 用户管理（需 ADMIN+）
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/admin/users` | 用户列表 |
| POST | `/api/admin/users` | 创建用户 |
| PUT | `/api/admin/users/:id` | 更新用户 |
| DELETE | `/api/admin/users/:id` | 删除用户（级联） |

### 签到账户
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/accounts` | 账户列表（支持 `?userId=` 筛选） |
| POST | `/api/accounts` | 创建账户 |
| GET | `/api/accounts/:id` | 账户详情 |
| PUT | `/api/accounts/:id` | 更新账户 |
| DELETE | `/api/accounts/:id` | 删除账户 |
| POST | `/api/accounts/:id/refresh-balance` | 刷新余额 |

### 签到记录
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/checkin-runs` | 签到历史 |
| POST | `/api/checkin-runs` | 手动执行签到 |
| POST | `/api/checkin-runs/cleanup` | 清理记录 |

### 设置（需 ADMIN+）
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/settings` | 获取全局设置 |
| PUT | `/api/settings` | 更新全局设置 |

## 配置

### 环境变量

```env
DATABASE_URL=sqlite:./data/ai-hub.db
TOKEN_ENCRYPTION_KEY=<base64 编码的 32 字节密钥>
# 生成: openssl rand -base64 32

PORT=8080                        # 服务端口
RUST_LOG=info                    # 日志级别
TZ=Asia/Shanghai                 # 时区
CORS_ALLOWED_ORIGINS=http://localhost:5173
SESSION_TTL_HOURS=24             # 会话有效期
ADMIN_USERNAME=admin             # 初始管理员用户名
ADMIN_PASSWORD=admin123456       # 初始管理员密码
```

### 1Panel 部署 CORS 配置

反向代理 + 前端静态文件时，`CORS_ALLOWED_ORIGINS` 留空或不设置即可。

## 权限矩阵

| 操作 | USER | ADMIN | SUPER_ADMIN |
|------|------|-------|-------------|
| 管理自己的账户 | ✅ | ✅ | ✅ |
| 查看所有账户 | ❌ | ✅ | ✅ |
| 管理用户 | ❌ | 仅 USER | ✅ |
| 管理 ADMIN | ❌ | ❌ | ✅ |
| 管理 SUPER_ADMIN | ❌ | ❌ | ❌ |
| 全局设置 | ❌ | ✅ | ✅ |

## 安全特性

- **登录保护** — 5 次失败后锁定 5 分钟，时序攻击防护（dummy bcrypt）
- **会话管理** — 内存 HashMap + TTL 自动过期，最大 1000 会话硬上限
- **加密存储** — Token/Cookie 使用 AES-256-GCM 加密后存入数据库
- **密码哈希** — bcrypt cost 10（1C1G 优化）
- **错误隔离** — 内部错误（DB/Crypto）不泄露详情给客户端，仅记录日志
- **所有权校验** — 所有账户操作检查 ownerId

## 1C1G 优化

- SQLite 连接池上限 5
- bcrypt cost 降为 10（平衡安全与性能）
- 共享 HTTP 客户端（OnceLock，30s 超时）
- 调度器并发上限 10（Semaphore）
- Docker 多阶段构建，CARGO_BUILD_JOBS=1
- 非 root 用户运行

## 技术栈

**后端：** Rust + Axum 0.7 + SQLite/sqlx + tokio-cron-scheduler + ring + bcrypt + reqwest

**前端：** Vue 3 + TypeScript + Vite

## 许可证

MIT
