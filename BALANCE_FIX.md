# 余额查询错误修复

## 修复内容

### 问题分析
从日志看到两类错误：
1. `余额请求失败：站点未返回 quota` - API 响应字段不匹配
2. 空消息错误 - 解密或网络异常

### 已修复的文件

#### 1. `src/services/checkin/providers/new_api.rs`
- ✅ 支持多种余额字段：`quota`, `data`, `data.quota`, `balance`, `credit`, `amount`
- ✅ 添加详细错误日志（包含响应体前 200 字符）
- ✅ HTTP 错误单独记录

#### 2. `src/services/checkin/providers/anyrouter.rs`
- ✅ 同样支持多种字段格式
- ✅ 增强错误日志

#### 3. `src/services/checkin/providers/x666.rs`
- ✅ 支持 `current_quota`, `quota`, `data.current_quota` 等字段
- ✅ 详细错误日志

#### 4. `src/routes/accounts.rs`
- ✅ 解密失败单独处理并记录
- ✅ 每个站点类型的错误分别记录
- ✅ 添加余额查询开始和成功的 INFO 日志

## 使用方法

### 1. 重新编译
```bash
cd rust-migration
cargo build --release
```

### 2. 启动服务
```bash
./target/release/ai-hub-rust
```

### 3. 测试余额查询
在前端点击"刷新余额"，观察控制台日志。

## 预期日志输出

### 成功时：
```
INFO Refreshing balance account_id="xxx" site_type="new-api"
INFO Balance refreshed successfully account_id="xxx" quota=100.5
```

### 失败时（字段不匹配）：
```
INFO Refreshing balance account_id="xxx" site_type="new-api"
ERROR Balance field not found in response: {"success":true,...}
ERROR New-API fetch_balance error: 余额请求失败：站点未返回余额字段。响应: {...}
```

### 失败时（解密错误）：
```
INFO Refreshing balance account_id="xxx" site_type="new-api"
ERROR Failed to decrypt access_token: ...
```

## 反馈信息

如果问题仍然存在，请提供：
1. **完整的错误日志**（从 "Refreshing balance" 到错误信息）
2. **账号的 siteType**
3. **站点的 baseUrl**
4. **响应体示例**（去掉敏感信息）

示例：
```
站点类型: new-api
baseUrl: https://api.example.com
响应: {"status": "ok", "user": {"credit": 100}}
```

## 常见问题

### Q: 看到 "站点未返回余额字段" 错误
**A**: 日志中会显示实际的响应，根据响应添加对应字段的解析。

### Q: 看到 "解密失败" 错误
**A**: TOKEN_ENCRYPTION_KEY 与创建账号时不一致，需要重新添加账号。

### Q: HTTP 401 错误
**A**: Token/Cookie 已过期，需要重新登录站点获取新的凭证。

## 修改说明

修复的核心逻辑：
```rust
// 之前：只尝试 quota 和 data
read_number(v.get("quota"))
    .or_else(|| v.get("data").and_then(|d| read_number(Some(d))))

// 现在：尝试多种常见字段
read_number(v.get("quota"))
    .or_else(|| v.get("data").and_then(|d| {
        if d.is_object() { read_number(d.get("quota")) }
        else { read_number(Some(d)) }
    }))
    .or_else(|| read_number(v.get("balance")))
    .or_else(|| read_number(v.get("credit")))
    .or_else(|| read_number(v.get("amount")))
```

这样可以兼容更多站点的 API 格式。
