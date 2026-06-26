# Rust + Axum 重构进度文档

## 项目状态

Rust 版本已从迁移阶段进入可维护阶段。当前实现包含 Axum 后端、SQLx + SQLite 数据层、Vue 3 + Vite 前端、Docker 多阶段构建、自动签到调度、通知配置与通知发送能力。

默认开发目录为 `rust-migration/`。

## 已完成范围

- 数据模型：`AppUser`、`CheckinAccount`、`CheckinRun`、`CheckinSetting`、`NotificationConfig`、`FailureCounter`。
- 数据层：SQLite 连接池、嵌入式 migration、camelCase 字段兼容、账户/用户/设置/签到记录/通知 CRUD。
- 认证与权限：登录、登出、当前用户、会话管理、管理员中间件、`USER` / `ADMIN` / `SUPER_ADMIN` 权限隔离。
- 核心 API：健康检查、服务器时间、账户管理、签到执行、批量签到、签到记录、统计、导入导出、全局设置、用户管理、通知配置。
- 签到系统：`new-api`、`anyrouter`、`x666` provider，余额刷新，失败重试，今日跳过，账户级并发锁。
- 调度器：时间窗口控制、串行执行、随机延迟、随机顺序、自动清理旧签到记录。
- 前端：账户管理、签到记录、统计、全局设置、用户管理、通知设置。
- 部署：Dockerfile、`docker-compose.hub.yml`、1C1G 部署脚本、静态资源由 Axum 服务。
- 测试：通知触发规则已有单元测试，后端通过 `cargo test` 与 `cargo clippy -- -D warnings`。

## 当前验证结果

最近验证命令：

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cd frontend && npm run build
```

验证结论：

- Rust 后端编译、Clippy、单元测试通过。
- Vue 前端类型检查与生产构建通过。
- 源码扫描未发现修复标记或未实现占位。

## 核心文件映射

| 原 Next.js 文件 | Rust 实现 | 状态 |
| --- | --- | --- |
| `prisma/schema.prisma` | `migrations/20260611_init.sql` | 完成 |
| `lib/prisma.ts` | `src/db/` | 完成 |
| `lib/crypto.ts` | `src/crypto.rs` | 完成 |
| `lib/checkin/runner.ts` | `src/services/checkin/runner.rs` | 完成 |
| `lib/checkin/scheduler.ts` | `src/services/scheduler.rs` | 完成 |
| `lib/checkin/new-api.ts` | `src/services/checkin/providers/new_api.rs` | 完成 |
| `app/api/accounts/route.ts` | `src/routes/accounts.rs` | 完成 |
| `app/api/auth/*/route.ts` | `src/routes/auth.rs` | 完成 |
| `app/api/settings/route.ts` | `src/routes/settings.rs` | 完成 |

## 后续优化方向

这些不是当前阻塞项：

- 为路由和数据库层补充更多集成测试。
- 如果需要加密 SMTP、STARTTLS 或更多邮件供应商兼容性，引入专用邮件库。
- 持续监控 1C1G 部署下的内存、SQLite 文件增长和通知发送延迟。
