# 管理员功能说明

本文档说明 Rust 版 AI Hub 当前的管理员能力、权限边界和常用 API。

## 角色模型

系统角色分为三类：

| 角色 | 说明 |
|------|------|
| `USER` | 普通用户，只能管理自己的账户、签到记录、统计和通知配置 |
| `ADMIN` | 管理员，可管理普通用户，可查看和操作全部账户与记录 |
| `SUPER_ADMIN` | 超级管理员，可创建和管理 `ADMIN`，但不能删除或降级 `SUPER_ADMIN` |

权限原则：

- 普通用户所有业务数据都按 `ownerId` 隔离。
- 管理员接口需要登录态和管理员权限中间件双重校验。
- `SUPER_ADMIN` 不能被删除或降级。
- `ADMIN` 不能管理其他 `ADMIN` 或 `SUPER_ADMIN`。

## 认证方式

当前版本使用 Cookie 会话，不再使用前端保存的 Bearer token。

登录成功后，后端会设置两个 Cookie：

- `session_id`：`HttpOnly`，用于服务端会话认证。
- `csrf_token`：用于前端读取，并在非安全请求中设置 `X-CSRF-Token`。

前端 `request()` 会自动：

- 携带 Cookie：`credentials: 'include'`
- 对 `POST` / `PUT` / `DELETE` / `PATCH` 注入 `X-CSRF-Token`
- 遇到非登录探测接口的 401 时触发会话过期提示

## 管理员能力

### 用户管理

管理员页面会显示：

- 用户名、角色、启用状态、备注
- 账户总数
- 启用账户数
- 最近失败账户数
- 最近签到时间

常用接口：

```http
GET /api/admin/users
GET /api/admin/users?scope=all
POST /api/admin/users
GET /api/admin/users/:id
PUT /api/admin/users/:id
DELETE /api/admin/users/:id
```

创建用户示例：

```json
{
  "username": "user1",
  "password": "password123",
  "role": "USER",
  "enabled": true,
  "note": "业务账号"
}
```

更新用户示例：

```json
{
  "password": "new_password",
  "role": "ADMIN",
  "enabled": false,
  "note": "临时停用"
}
```

### 账户管理

管理员可以：

- 查看全部用户的账户。
- 按用户、站点类型、启用状态、签到状态、关键词筛选。
- 按用户分组查看账户，当前用户分组置顶。
- 对当前列表、单个分组或选中账户执行批量签到。
- 批量刷新余额、批量启用、批量禁用。
- 导入和导出 CSV。

常用接口：

```http
GET /api/accounts
GET /api/accounts?userId=:user_id
GET /api/accounts?siteType=new-api&enabled=true&lastStatus=failed&keyword=note
POST /api/accounts
PUT /api/accounts/:id
DELETE /api/accounts/:id
POST /api/accounts/:id/refresh-balance
GET /api/accounts/export
POST /api/accounts/import
```

更新账户时，可选字段支持三态语义：

- 字段缺失：保持原值。
- 字段为 `null`：清空为数据库 `NULL`。
- 字段为字符串：更新为新值。

示例：

```json
{
  "name": "账号 A",
  "userId": null,
  "customCheckinUrl": null,
  "note": null,
  "enabled": true
}
```

### 签到记录

管理员可以：

- 查看所有用户签到记录。
- 按用户、账户、状态、触发方式、时间范围筛选。
- 手动执行任意账户签到。
- 对当前失败账户批量重试。
- 对单条失败记录重试。
- 复制签到摘要。
- 清理全部记录或保留最新 N 条。

常用接口：

```http
GET /api/checkin-runs
GET /api/checkin-runs?userId=:user_id&status=failed
POST /api/checkin-runs
POST /api/checkin-runs/batch
POST /api/checkin-runs/cleanup
```

单次签到：

```json
{
  "accountId": "account_id_here"
}
```

批量签到：

```json
{
  "accountIds": ["account_id_1", "account_id_2"]
}
```

清理记录：

```json
{
  "keepLatest": 500
}
```

`keepLatest = 0` 表示清空全部记录。

### 数据统计

管理员可以查看全局统计，也可以按用户筛选。

```http
GET /api/statistics
GET /api/statistics?startDate=2026-06-01&endDate=2026-06-29
GET /api/statistics?userId=:user_id
```

统计内容包括：

- 总账户数、启用账户数
- 今日成功、今日失败
- 区间总执行次数和成功率
- 总余额
- 每日趋势
- 站点统计
- 最近失败记录

普通用户即使手动传入其他 `userId`，后端也会返回 `403`。

### 通知配置

通知配置属于当前登录用户。管理员可以配置自己的通知规则，普通用户也可以配置自己的通知规则。

支持类型：

- `email`
- `webhook`
- `telegram`

常用接口：

```http
GET /api/notifications
POST /api/notifications
GET /api/notifications/:id
PUT /api/notifications/:id
DELETE /api/notifications/:id
POST /api/notifications/:id/test
```

更新通知时，以下字段支持 `null` 清空：

- `balanceThreshold`
- `webhookHeaders`
- `note`

示例：

```json
{
  "onBalanceLow": false,
  "balanceThreshold": null,
  "webhookHeaders": null,
  "note": null
}
```

### 全局设置

仅 `ADMIN` 和 `SUPER_ADMIN` 可以查看和修改全局设置。

```http
GET /api/settings
PUT /api/settings
```

示例：

```json
{
  "enabled": true,
  "windowStart": "02:00",
  "windowEnd": "05:00",
  "retryEnabled": true,
  "maxAttemptsPerDay": 3,
  "batchDelayMin": 3,
  "batchDelayMax": 10,
  "cleanupKeepLatest": 500
}
```

字段说明：

| 字段 | 说明 |
|------|------|
| `enabled` | 是否启用自动签到 |
| `windowStart` / `windowEnd` | 自动签到时间窗口，本地时间，格式 `HH:MM` |
| `retryEnabled` | 是否启用失败重试 |
| `maxAttemptsPerDay` | 每个账户每天最大尝试次数，范围 `1~100` |
| `batchDelayMin` / `batchDelayMax` | 批量和定时签到的账户间随机延迟，范围 `0~600` 秒 |
| `cleanupKeepLatest` | 定时清理保留的最新记录数，范围 `0~10000`，`0` 表示清空全部 |

后端接收前端 camelCase 字段，同时兼容 snake_case 字段。

## 权限对比表

| 功能 | USER | ADMIN | SUPER_ADMIN |
|------|------|-------|-------------|
| 登录/登出 | ✅ | ✅ | ✅ |
| 管理自己的账户 | ✅ | ✅ | ✅ |
| 查看所有账户 | ❌ | ✅ | ✅ |
| 批量操作所有账户 | ❌ | ✅ | ✅ |
| 查看自己的签到记录 | ✅ | ✅ | ✅ |
| 查看所有签到记录 | ❌ | ✅ | ✅ |
| 查看自己的统计 | ✅ | ✅ | ✅ |
| 查看全局统计 | ❌ | ✅ | ✅ |
| 管理自己的通知 | ✅ | ✅ | ✅ |
| 管理普通用户 | ❌ | ✅ | ✅ |
| 管理 ADMIN | ❌ | ❌ | ✅ |
| 管理 SUPER_ADMIN | ❌ | ❌ | ❌ |
| 修改全局设置 | ❌ | ✅ | ✅ |

## 安全特性

- 登录失败频率限制：5 次失败后短时间锁定。
- 用户不存在时执行 dummy bcrypt 校验，降低用户名枚举风险。
- 密码只存储 bcrypt 哈希。
- 账户 token / cookie 使用 AES-256-GCM 加密存储。
- 非安全请求需要 CSRF header。
- 会话有 TTL，且存在最大会话数硬上限。
- 数据库查询使用参数绑定，避免 SQL 注入。
- 用户级数据在数据库和路由层按 `ownerId` 过滤。

## 常见问题

### 如何添加第二个管理员？

使用 `SUPER_ADMIN` 创建用户并设置 `role = "ADMIN"`。

### 普通管理员能创建管理员吗？

不能。`ADMIN` 只能创建或管理 `USER`。

### 如何禁用用户而不删除数据？

调用 `PUT /api/admin/users/:id`，设置：

```json
{
  "enabled": false
}
```

禁用用户后，调度器不会处理该用户拥有的账户。

### 普通用户可以查看其他用户账户吗？

不可以。账户、签到记录和统计都会按当前用户 `ownerId` 过滤。

### API 调试时为什么 POST/PUT/DELETE 返回 401 或 403？

确认两点：

1. 请求携带登录后的 `session_id` Cookie。
2. 非安全请求带有 `X-CSRF-Token`，其值等于 `csrf_token` Cookie。
