# 管理员账号设置指南

## 默认管理员账号

**首次启动时会自动创建默认管理员账号**:

- **用户名**: `admin`
- **密码**: `admin123`

⚠️ **安全警告**: 请在首次登录后立即修改密码!

## 自定义管理员账号

### 方式 1: 环境变量 (推荐)

在 `.env` 文件中设置:

```env
ADMIN_USERNAME=your_admin_username
ADMIN_PASSWORD=your_secure_password
```

### 方式 2: Docker 环境变量

在 `docker-compose.yml` 中添加:

```yaml
services:
  app:
    environment:
      - ADMIN_USERNAME=your_admin_username
      - ADMIN_PASSWORD=your_secure_password
```

### 方式 3: 启动时指定

```bash
ADMIN_USERNAME=myuser ADMIN_PASSWORD=mypass cargo run
```

或 Docker:

```bash
docker run -e ADMIN_USERNAME=myuser -e ADMIN_PASSWORD=mypass ...
```

## 修改管理员密码

### 通过 API

```bash
curl -X PUT http://localhost:3000/api/admin/users/{user_id} \
  -H "Content-Type: application/json" \
  -d ''{
    "password": "new_secure_password"
  }''
```

### 通过前端界面

1. 登录管理员账号
2. 进入用户管理页面
3. 编辑管理员用户
4. 修改密码并保存

## 创建额外管理员

```bash
curl -X POST http://localhost:3000/api/admin/users \
  -H "Content-Type: application/json" \
  -d ''{
    "username": "admin2",
    "password": "secure_password",
    "role": "ADMIN"
  }''
```

## 安全建议

1. ✅ **立即修改默认密码**
2. ✅ 使用强密码 (至少 12 位,包含大小写字母、数字、符号)
3. ✅ 定期更换密码
4. ✅ 不要在代码或日志中暴露密码
5. ✅ 使用 HTTPS (生产环境)

## 密码规范建议

**强密码示例**:
- `MyS3cur3P@ssw0rd!2024`
- `Tr0ngP@$$word#AI_Hub`
- `Adm1n!Secur3#2024`

**弱密码 (避免使用)**:
- `admin`
- `123456`
- `password`
- `admin123` (默认密码)

## 忘记密码?

### 方式 1: 重置通过环境变量

1. 停止服务
2. 删除数据库中的用户记录
3. 设置新的 `ADMIN_PASSWORD`
4. 重启服务 (会自动重新创建)

### 方式 2: 直接修改数据库

```bash
# 生成新密码哈希
echo "new_password" | bcrypt-cli

# 更新数据库
sqlite3 data/ai-hub.db "UPDATE AppUser SET passwordHash = ''your_hash'' WHERE username = ''admin''"
```

### 方式 3: 删除数据库重新开始

⚠️ **这会丢失所有数据!**

```bash
rm data/ai-hub.db
# 重启服务会重新创建数据库和管理员账号
```

## 角色说明

- **ADMIN**: 管理员,拥有所有权限
- **USER**: 普通用户,仅能管理自己的签到账号

## 快速开始

```bash
# 1. 配置管理员账号 (可选)
cp .env.example .env
vim .env  # 设置 ADMIN_USERNAME 和 ADMIN_PASSWORD

# 2. 启动服务
docker-compose up -d

# 3. 登录
curl -X POST http://localhost:3000/api/auth/login \
  -H "Content-Type: application/json" \
  -d ''{
    "username": "admin",
    "password": "admin123"
  }''

# 4. 立即修改密码!
```

## 常见问题

### Q: 管理员账号何时创建?
A: 首次启动服务时自动创建,如果已存在则不会重复创建。

### Q: 可以有多个管理员吗?
A: 可以,通过 API 或界面创建额外的 ADMIN 角色用户。

### Q: 环境变量修改后需要重启吗?
A: 是的,管理员账号仅在服务启动时初始化。

### Q: 默认密码会在日志中显示吗?
A: 首次创建时会在日志中警告显示,提醒您修改密码。

---

**重要**: 生产环境务必修改默认密码! 🔒
