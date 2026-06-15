# 更新日志

## v2.2.1 (2026-06-15)

### Bug 修复

- **修复余额查询响应乱码** — reqwest 开启 `gzip`/`brotli`/`deflate` 特性后自动解压响应体，解决站点返回 gzip 压缩数据导致 `serde_json` 解析失败、报"站点未返回 quota"的问题
- **修复 UTF-8 字节切片 panic** — new_api / arrouter / x666 三个 provider 中 `&text[..200]` 改为 `text.chars().take(200).collect()`，避免在多字节字符（中文）中间切断导致线程崩溃
- **修复前端余额显示错误** — `AccountPanel.vue` 的 `formatBalance` 之前直接把 `quota` 当美元，现按 One API 标准 `quota / 500000 = USD` 换算，与 Next.js 版本（`QUOTA_PER_USD = 500000`）完全对齐

### 增强

- **反爬求解诊断日志** — `solve_acw_sc_v2` 各失败点（arg1 长度不匹配、非 hex 字符）及签到/余额查询两处调用点添加 `warn` 日志，便于将来 WAF 算法升级时快速定位（含 arg1 实际长度、期望长度、预览）

### 清理

- 删除误产生的 3 字节垃圾文件 `arrouter.rs`

---

## v2.2.0 (2026-06-15)

### 签到逻辑完善（参考 React 项目对齐）

- **x666 签到增强**
  - JSON 解析容错：处理 HTML 404 页面和非 JSON 响应
  - 优先检测"已签到"状态，避免误判
  - 优化错误消息提取逻辑（message/error 字段）
  - 余额查询支持字符串和数字类型的 current_quota

- **anyrouter 签到增强**
  - 成功消息检测支持英文 "success" 和中文 "签到成功"
  - JSON 解析失败时使用原始响应文本（不再返回空消息）
  - 保持 acw_sc__v2 反爬验证码自动求解逻辑

- **anyrouter 余额查询新增** 🎉
  - 实现 `fetch_balance()` 函数，使用 `/api/user/self` 端点
  - 自动处理反爬挑战页面（acw_sc__v2 自动重试）
  - 支持从 `quota` 或 `data` 字段提取余额
  - refresh_balance API 现在支持三种账号类型（new-api/anyrouter/x666）

### 账号管理增强

- **输入验证逻辑**
  - anyrouter 账号必须提供 userId 和 cookie
  - x666 账号必须提供 cookie
  - new-api 使用 access_token 认证时必须提供 accessToken
  - 更新账号时防止清除必需的凭证字段

- **authType 自动调整**
  - 创建 anyrouter 或 x666 账号时自动设置 authType 为 "cookie"
  - 避免用户手动配置错误

### 错误处理改进

- 修复：`expected value at line 1 column 1` 导致的 500 错误
- 修复：401/404 等 HTTP 错误返回的错误消息正确提取和显示
- 增强：所有 provider 统一的错误处理模式

---

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
