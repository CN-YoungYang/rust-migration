# Rust + Axum 重构进度文档

## 项目概述

从 Next.js + Prisma + SQLite 迁移到 Rust + Axum + SQLx/Diesel + SQLite。

**目标**:
- 更高性能和更低资源占用
- 类型安全的编译时检查
- 更适合长期运行的后台任务
- 保持现有功能完整性

## 技术栈选择

- **Web框架**: Axum (基于 Tokio 的高性能异步框架)
- **数据库**: SQLite + SQLx (编译时 SQL 验证)
- **序列化**: Serde
- **加密**: ring / RustCrypto
- **HTTP客户端**: reqwest
- **任务调度**: tokio-cron-scheduler
- **日志**: tracing + tracing-subscriber

## 重构阶段

### 阶段 1: 项目初始化 ✅
- [x] 创建 Rust workspace 目录结构
- [x] 编写 Cargo.toml 配置
- [x] 设置项目结构 (src/models, src/routes, src/services)
- [x] 创建进度文档

### 阶段 2: 数据层迁移 🔄
- [ ] 定义 Rust 数据模型 (AppUser, CheckinAccount, CheckinRun, CheckinSetting)
- [ ] 创建 SQLx migrations
- [ ] 实现数据库连接池
- [ ] 实现加密/解密工具函数
- [ ] 实现基础 CRUD 操作

### 阶段 3: 核心 API 路由 ⏳
- [ ] /api/health - 健康检查
- [ ] /api/server-time - 服务器时间
- [ ] /api/auth/* - 登录/登出/当前用户
- [ ] /api/accounts - 签到账号管理
- [ ] /api/accounts/[id] - 单个账号操作
- [ ] /api/settings - 全局设置
- [ ] /api/checkin-runs - 签到记录
- [ ] /api/admin/users - 用户管理

### 阶段 4: 签到业务逻辑 ⏳
- [ ] New API provider
- [ ] AnyRouter provider  
- [ ] X666 provider
- [ ] 签到执行器 (runner)
- [ ] 余额查询功能
- [ ] 错误处理和重试逻辑

### 阶段 5: 定时任务调度 ⏳
- [ ] 实现定时签到调度器
- [ ] 时间窗口控制
- [ ] 失败重试机制
- [ ] 并发控制和任务锁

### 阶段 6: Docker 与部署 ⏳
- [ ] 编写 Dockerfile (多阶段构建)
- [ ] 编写 docker-compose.yml
- [ ] 环境变量配置
- [ ] 数据持久化配置

### 阶段 7: 文档与测试 ⏳
- [ ] API 文档
- [ ] 部署说明
- [ ] 迁移指南
- [ ] 单元测试 (关键业务逻辑)

## 当前进度

**当前阶段**: 阶段 1 - 项目初始化
**完成度**: 5%
**预计完成时间**: 进行中

## 最近更新

### 2026-06-11
- 创建项目目录结构
- 编写进度文档框架
- 开始 Cargo.toml 配置

## 核心文件映射

| 原 Next.js 文件 | 新 Rust 文件 | 状态 |
|----------------|-------------|------|
| prisma/schema.prisma | migrations/*.sql | ⏳ |
| lib/prisma.ts | src/db.rs | ⏳ |
| lib/crypto.ts | src/crypto.rs | ⏳ |
| lib/checkin/runner.ts | src/services/checkin/runner.rs | ⏳ |
| lib/checkin/scheduler.ts | src/services/scheduler.rs | ⏳ |
| lib/checkin/new-api.ts | src/services/checkin/providers/new_api.rs | ⏳ |
| app/api/accounts/route.ts | src/routes/accounts.rs | ⏳ |
| app/api/auth/*/route.ts | src/routes/auth.rs | ⏳ |
| app/api/settings/route.ts | src/routes/settings.rs | ⏳ |

## 注意事项

1. **保持功能一致性**: 确保所有现有功能在 Rust 版本中都能正常工作
2. **API 兼容性**: 保持 REST API 端点和数据格式不变,便于前端复用
3. **数据迁移**: SQLite 数据库可直接复用,只需确保表结构一致
4. **环境变量**: 保持与 Next.js 版本相同的环境变量命名
5. **错误处理**: Rust 的 Result 类型提供更安全的错误处理

## 性能目标

- 启动时间: < 1s
- 内存占用: < 50MB (空闲状态)
- API 响应: < 10ms (P95)
- 签到执行: 保持原有成功率

## 下一步行动

1. 初始化 Cargo workspace
2. 创建基础目录结构
3. 编写数据模型定义
4. 实现数据库连接
5. 开始迁移核心 API 路由
