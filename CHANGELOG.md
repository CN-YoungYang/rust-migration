# 更新日志

## v2.1.0 (2026-06-13)

### 安全加固

- **登录时序攻击防护** — 用户不存在时也执行 dummy bcrypt 校验，消除用户名枚举风险
- **登录频率限制统一** — 所有登录失败（用户不存在、密码错误、账户禁用）均记录频率
- **设置接口权限修复** — GET/PUT `/api/settings` 添加 admin 角色校验，之前任何登录用户都能修改全局设置
- **错误信息隔离** — 内部错误（数据库、加密）不再向客户端泄露详情，仅在服务端日志记录
- **所有权校验** — 所有账户操作（查看、编辑、删除、刷新余额）检查 ownerId

### 功能修复

- **全局重试开关生效** — 调度器现在同时检查全局 `retryEnabled` 和账户级别设置（之前全局开关无效）
- **手动签到次数限制** — `/api/checkin-runs` POST 端点现在执行 `maxAttemptsPerDay` 校验
- **余额查询 userId 传递** — `new_api::fetch_balance` 现在正确发送 userId 相关 headers（之前被忽略）
- **更新接口返回值** — PUT `/api/accounts/:id` 现在返回完整账户 JSON（之前只返回 `{success: true}`）
- **用户名重复检查** — 创建用户前检查用户名是否已存在，返回清晰验证错误而非 500

### 性能优化

- **调度器并发控制** — 使用 `Semaphore(10)` 限制同时执行的签到任务数（之前无限制 spawn）
- **共享 HTTP 客户端** — OnceLock 全局复用 reqwest Client，30s 超时
- **数据库索引** — 新增 `CheckinRun(createdAt)` 索引，优化管理员全局记录查询
- **bcrypt cost 10** — 从 cost 12 降为 10，减少 1C1G 服务器登录耗时
- **SQL 简化** — `cleanup_checkin_runs` 使用子查询替代动态占位符拼接

### 时区修复

- **统一 UTC 存储** — 数据库所有时间戳使用 `DateTime<Utc>` 存储
- **本地窗口比较** — 调度器将 UTC 时间转换为本地时间后比较签到窗口和"今日"
- **计数查询修正** — `count_runs_by_account_today` 将本地午夜转换为 UTC 后查询，确保跨午夜正确
- **迁移格式统一** — 默认设置的时间戳改用 RFC 3339 格式（`strftime('%Y-%m-%dT%H:%M:%SZ')`）

### 代码重构

- **account_to_json** — 提取公共 JSON 构建函数，消除 accounts.rs 中 3 处重复
- **classify_checkin_status** — 提取签到状态分类到 `providers/mod.rs`，new_api 和 anyrouter 共用
- **前端 API 封装** — 提取 `api.ts`（getToken/authHeaders/request/apiUrl），4 个组件共用
- **错误响应解析** — `request()` 现在解析 JSON 错误体显示 `error/message/details` 字段
- **currentUser 传递** — AdminUserPanel 通过 props 接收 currentUser，移除重复 `/auth/me` 请求
- **CSS 清理** — style.css 移除 100+ 行无用 glassmorphism 样式，只保留 CSS reset + 动画

### 数据库

- `CheckinRun` 新增 `createdAt` 单列索引
- `CheckinAccount(ownerId)` 索引（已有）
- 迁移文件合并为单文件 `20260611_init.sql`（幂等 `IF NOT EXISTS`）

---

## v2.0.0 (2026-06-12)

### 新功能

- **用户管理** — 完整 CRUD，USER/ADMIN/SUPER_ADMIN 三级权限
- **所有权隔离** — `ownerId` 字段关联账户创建者
- **余额查询** — 支持 new-api 和 x666 站点实时查询
- **记录清理** — 管理员清理全部，普通用户清理自己的
- **自动调度** — Cron 签到窗口、重试策略、每日上限

### API 变更

- 新增 `POST /api/accounts/:id/refresh-balance`
- 新增 `POST /api/checkin-runs/cleanup`
- `GET /api/accounts` 支持 `?userId=` 筛选

---

## v1.0.0 (2026-06-10)

初始 Rust 版本发布
