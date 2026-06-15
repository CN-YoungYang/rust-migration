# 余额查询故障排查指南

## 已修复的问题

### 1. 增强的字段解析
现在支持多种常见的余额字段名：
- `quota`
- `data` (对象或数字)
- `data.quota`
- `balance`
- `credit`
- `amount`
- `current_quota` (x666)

### 2. 详细的错误日志
现在会记录：
- HTTP 状态码
- 完整的响应体（前 200 字符）
- 具体是哪个字段缺失
- 解密失败的详细信息

### 3. 更好的错误处理
- 解密失败单独处理
- 每个站点类型的错误分别记录

## 测试步骤

### 1. 重新编译并启动
```bash
cd rust-migration
cargo build --release
./target/release/ai-hub-rust
# 或
cargo run
```

### 2. 查看日志级别
确保 `.env` 文件中：
```env
RUST_LOG=info
```

如果需要更详细的调试信息：
```env
RUST_LOG=debug
```

### 3. 测试余额查询
在前端点击"刷新余额"按钮，观察服务器日志输出。

### 4. 预期的日志格式

#### 成功情况：
```
INFO ai_hub_rust::routes::accounts: Refreshing balance account_id="xxx" site_type="new-api"
INFO ai_hub_rust::routes::accounts: Balance refreshed successfully account_id="xxx" quota=100.5
```

#### 失败情况（字段缺失）：
```
INFO ai_hub_rust::routes::accounts: Refreshing balance account_id="xxx" site_type="new-api"
ERROR ai_hub_rust::services::checkin::providers::new_api: Balance field not found in response: {"success":true,"data":{"username":"test"}}
ERROR ai_hub_rust::routes::accounts: New-API fetch_balance error: 余额请求失败：站点未返回余额字段。响应: {"success":true,"data":{"username":"test"}}
```

#### 失败情况（HTTP 错误）：
```
INFO ai_hub_rust::routes::accounts: Refreshing balance account_id="xxx" site_type="new-api"
ERROR ai_hub_rust::services::checkin::providers::new_api: Balance fetch failed: HTTP 401, body: {"error":"Unauthorized"}
ERROR ai_hub_rust::routes::accounts: New-API fetch_balance error: 余额请求失败：HTTP 401
```

## 常见问题排查

### 问题 1: "余额请求失败：站点未返回余额字段"

**原因**: API 返回的 JSON 结构不包含任何已知的余额字段

**解决方案**:
1. 查看日志中的完整响应体
2. 找到实际的余额字段名
3. 修改对应的 `fetch_balance` 函数添加该字段

**示例**: 如果响应是 `{"data": {"user_quota": 100}}`
```rust
// 在 new_api.rs 中添加：
.or_else(|| v.get("data").and_then(|d| read_number(d.get("user_quota"))))
```

### 问题 2: "解密失败"

**原因**: TOKEN_ENCRYPTION_KEY 不正确或数据损坏

**解决方案**:
1. 确认 `.env` 中的 `TOKEN_ENCRYPTION_KEY` 与创建账号时使用的密钥一致
2. 如果密钥更改过，需要重新添加账号

### 问题 3: HTTP 401/403 错误

**原因**: Token 或 Cookie 已过期

**解决方案**:
1. 重新登录对应的站点
2. 复制新的 Token/Cookie
3. 更新账号信息

### 问题 4: 空消息错误

**原因**: 可能是网络超时或解密异常

**现在会显示具体原因**:
- 如果是解密失败，会显示 "解密失败"
- 如果是网络错误，会显示具体的错误信息

## 手动测试 API

使用 curl 测试：

```bash
# 1. 登录获取 token
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123456"}'

# 2. 获取账户列表
curl http://localhost:8080/api/accounts \
  -H "Authorization: Bearer YOUR_TOKEN"

# 3. 刷新指定账户余额
curl -X POST http://localhost:8080/api/accounts/ACCOUNT_ID/refresh-balance \
  -H "Authorization: Bearer YOUR_TOKEN"
```

## 如何提供反馈

如果问题仍然存在，请提供：
1. 完整的错误日志（包括 ERROR 和 INFO 行）
2. 账号的 siteType（new-api / anyrouter / x666）
3. 站点的 baseUrl
4. 是否能手动在浏览器访问余额接口（去掉敏感信息）

## 下一步优化建议

如果你经常使用特定的站点，可以：
1. 在日志中找到该站点返回的完整 JSON 格式
2. 告诉我具体的字段结构
3. 我可以为该站点添加专门的解析逻辑
