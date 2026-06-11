# 管理员功能说明

## 管理员能做什么

### 1. 用户管理 👥

**查看所有用户**
```bash
GET /api/admin/users
Authorization: Bearer {admin_token}
```

**创建新用户**
```bash
POST /api/admin/users
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "username": "newuser",
  "password": "password123",
  "role": "USER"  # 或 "ADMIN"
}
```

**修改用户信息**
```bash
PUT /api/admin/users/{user_id}
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "password": "new_password",  # 可选
  "role": "ADMIN",             # 可选
  "enabled": false,            # 可选 - 禁用用户
  "note": "备注信息"           # 可选
}
```

**删除用户**
```bash
DELETE /api/admin/users/{user_id}
Authorization: Bearer {admin_token}
```

### 2. 查看所有签到账号 📝

**管理员可以查看和管理所有用户的签到账号**
```bash
GET /api/accounts
Authorization: Bearer {admin_token}
```

### 3. 管理全局设置 ⚙️

**查看设置**
```bash
GET /api/settings
Authorization: Bearer {admin_token}
```

**修改设置**
```bash
PUT /api/settings
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "enabled": true,           # 开启自动签到
  "windowStart": "02:00",    # 签到时间窗口开始
  "windowEnd": "05:00",      # 签到时间窗口结束
  "retryEnabled": true,      # 开启失败重试
  "maxAttemptsPerDay": 3     # 每日最大重试次数
}
```

### 4. 查看所有签到记录 📊

```bash
GET /api/checkin-runs
Authorization: Bearer {admin_token}
```

### 5. 手动触发签到 🚀

```bash
POST /api/checkin-runs
Authorization: Bearer {admin_token}
Content-Type: application/json

{
  "account_id": "account_id_here"
}
```

## 普通用户能做什么

### 1. 管理自己的签到账号 📋

- 查看自己的账号列表
- 添加新账号
- 修改自己的账号
- 删除自己的账号

### 2. 手动执行签到 ▶️

- 对自己的账号执行签到

### 3. 查看签到记录 📈

- 查看自己账号的签到历史

## 权限对比表

| 功能 | 普通用户 | 管理员 |
|------|---------|----------|
| 登录/登出 | ✅ | ✅ |
| 管理自己的签到账号 | ✅ | ✅ |
| 查看所有用户的账号 | ❌ | ✅ |
| 手动执行签到 | ✅ (仅自己) | ✅ (所有) |
| 查看签到记录 | ✅ (仅自己) | ✅ (所有) |
| 管理用户 | ❌ | ✅ |
| 创建用户 | ❌ | ✅ |
| 修改用户密码 | ❌ | ✅ |
| 禁用/删除用户 | ❌ | ✅ |
| 修改全局设置 | ❌ | ✅ |
| 升级用户为管理员 | ❌ | ✅ |

## API 认证流程

### 1. 登录获取 Token

```bash
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d ''{
    "username": "admin",
    "password": "admin123"
  }''

# 响应:
{
  "token": "uuid-token-here",
  "user": {
    "id": "...",
    "username": "admin",
    "role": "ADMIN",
    "enabled": true
  }
}
```

### 2. 使用 Token 访问受保护的 API

```bash
curl -X GET http://localhost:3000/api/accounts \
  -H "Authorization: Bearer {your-token}"
```

### 3. 登出

```bash
curl -X POST http://localhost:3000/api/auth/logout \
  -H "Authorization: Bearer {your-token}"
```

## 安全特性

### ✅ 已实现

1. **身份验证**: 所有 API (除登录/健康检查) 需要 Token
2. **角色隔离**: ADMIN 和 USER 权限严格分离
3. **密码哈希**: 使用 bcrypt 加密存储
4. **Token 加密**: AES-256-GCM 加密
5. **参数化查询**: 防止 SQL 注入
6. **用户禁用**: 可禁用用户而不删除数据

### ⚠️ 建议改进 (生产环境)

1. **使用 HTTPS**: 生产环境必须
2. **Token 过期**: 当前 token 永不过期
3. **频率限制**: 防止暴力攻击
4. **CORS 限制**: 限制允许的来源域名

## 使用示例

### 管理员工作流

```bash
# 1. 登录
TOKEN=$(curl -s -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d ''{ "username": "admin", "password": "admin123" }'' \
  | jq -r ''.token'')

# 2. 创建普通用户
curl -X POST http://localhost:3000/api/admin/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d ''{
    "username": "user1",
    "password": "user123",
    "role": "USER"
  }''

# 3. 查看所有用户
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/admin/users

# 4. 修改全局设置
curl -X PUT http://localhost:3000/api/settings \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d ''{
    "enabled": true,
    "windowStart": "02:00",
    "windowEnd": "05:00"
  }''

# 5. 查看所有签到记录
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/api/checkin-runs
```

## 常见问题

### Q: 如何添加第二个管理员?
A: 使用现有管理员账号调用 `POST /api/admin/users`,设置 `role` 为 `"ADMIN"`。

### Q: 普通用户可以升级为管理员吗?
A: 只有管理员可以通过 `PUT /api/admin/users/{id}` 修改用户角色。

### Q: 如何禁用用户而不删除?
A: 使用 `PUT /api/admin/users/{id}`,设置 `enabled: false`。

### Q: Token 会过期吗?
A: 当前版本 token 不会过期。生产环境建议实现 JWT 并设置过期时间。

### Q: 普通用户可以查看其他用户的账号吗?
A: 不可以。每个用户只能查看和管理自己的签到账号。

---

**总结**: 管理员拥有平台全部权限,可以管理用户、查看所有数据、配置系统设置。
