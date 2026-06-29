# 管理员账号设置指南

## 初始管理员

首次启动时，如果数据库中还没有 `ADMIN_USERNAME` 对应用户，系统会自动创建 `SUPER_ADMIN`。

`.env` 示例：

```env
ADMIN_USERNAME=admin
ADMIN_PASSWORD=YourSecurePassword123!@#
```

要求：

- `ADMIN_PASSWORD` 首次创建管理员时必填。
- 密码至少 8 位。
- 生产环境必须使用强密码。
- 管理员创建后，密码以 bcrypt 哈希存储。
- 首次登录后建议修改密码，并从部署环境中移除 `ADMIN_PASSWORD`。

## Docker / 1Panel 配置

`docker-compose.hub.yml` 会读取 `.env`：

```yaml
env_file:
  - .env
```

生产部署时至少确认：

```env
TOKEN_ENCRYPTION_KEY=<openssl rand -base64 32 生成的值>
ADMIN_USERNAME=admin
ADMIN_PASSWORD=<强密码>
RUST_LOG=warn
TZ=Asia/Shanghai
COOKIE_SECURE=true
```

如果暂时没有 HTTPS，`COOKIE_SECURE` 需要保持 `false`，否则浏览器不会通过 HTTP 发送认证 Cookie。

## 修改管理员密码

推荐通过前端：

1. 使用 `SUPER_ADMIN` 登录。
2. 进入“用户管理”。
3. 编辑目标用户。
4. 输入新密码并保存。

API 调试时需要 Cookie 和 CSRF：

1. 登录并保存 Cookie。
2. 从 `csrf_token` Cookie 读取值。
3. `PUT /api/admin/users/:id` 时带上 `X-CSRF-Token`。

请求体示例：

```json
{
  "password": "NewSecurePassword123!"
}
```

## 创建额外管理员

只有 `SUPER_ADMIN` 可以创建 `ADMIN`。

请求体示例：

```json
{
  "username": "admin2",
  "password": "AnotherSecurePassword123!",
  "role": "ADMIN",
  "enabled": true,
  "note": "备用管理员"
}
```

`ADMIN` 只能创建或管理 `USER`，不能创建或管理其他管理员。

## 角色说明

| 角色 | 能力 |
|------|------|
| `USER` | 管理自己的账户、记录、统计和通知 |
| `ADMIN` | 管理普通用户，查看和操作全局账户与记录，修改全局设置 |
| `SUPER_ADMIN` | 管理普通用户和管理员，但不能被删除或降级 |

## 忘记密码

### 方式 1：使用另一个管理员修改

如果还有其他 `SUPER_ADMIN`，直接在“用户管理”中重置密码。

### 方式 2：删除用户后重建

适用于只有一个管理员且无法登录的情况。

1. 停止服务。
2. 备份 `data/ai-hub.db`。
3. 删除目标管理员记录。
4. 设置新的 `ADMIN_PASSWORD`。
5. 重启服务，让系统重新创建初始管理员。

### 方式 3：重建数据库

这会丢失所有数据，仅在测试环境使用：

```bash
Remove-Item .\data\ai-hub.db
```

Linux：

```bash
rm ./data/ai-hub.db
```

重启后会重新执行 migration 并创建管理员。

## 安全建议

- 使用 12 位以上强密码。
- 生产环境使用 HTTPS，并设置 `COOKIE_SECURE=true`。
- 不要把 `.env` 提交到 Git。
- 不要在日志、截图、工单中暴露密码、Cookie、token。
- 保持 `RUST_LOG=warn`，排查问题时临时切到 `debug`。
- 定期备份 SQLite 数据库。
