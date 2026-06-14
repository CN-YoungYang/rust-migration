# 签到 Provider 实现说明

本文档说明各站点类型的签到和余额查询实现细节。

## 支持的站点类型

| 站点类型 | 签到端点 | 余额查询端点 | 认证方式 | 特殊处理 |
|---------|---------|------------|---------|---------|
| new-api | `/api/user/checkin` | `/api/user/self` | Bearer Token / Cookie | - |
| anyrouter | `/api/user/sign_in` | `/api/user/self` | Cookie + User-id header | acw_sc__v2 反爬 |
| x666 | `/api/checkin/spin` | `/api/checkin/status` | Cookie | - |

## 实现细节

### new-api

**签到逻辑** (`src/services/checkin/providers/new_api.rs`)
- 发送 POST `{baseUrl}/api/user/checkin` + Bearer Token
- 支持多种站点兼容 headers（New-API-User, Veloera-User 等）
- 解析 `success` 和 `message` 字段判断状态

**余额查询**
- GET `{baseUrl}/api/user/self`
- 从 `quota` 或 `data.quota` 字段读取余额
- 支持字符串和数字类型

### anyrouter

**签到逻辑** (`src/services/checkin/providers/anyrouter.rs`)
- POST `{baseUrl}/api/user/sign_in` + Cookie + User-id header
- **反爬处理**：检测 HTML 中的 `acw_sc__v2` 和 `var arg1`
  - 提取 arg1 hex 字符串
  - 按索引表重排序并与密钥 XOR
  - 自动在 Cookie 中添加 `acw_sc__v2={解算值}` 重试
- 空消息判定为"已签到"（anyrouter 特性）
- 支持 `success` 字段和消息关键词检测

**余额查询** ✨ v2.2.0 新增
- GET `{baseUrl}/api/user/self` + Cookie + User-id header
- 同样支持反爬挑战自动重试
- 从 `quota` 或 `data` 字段提取余额
- 失败时返回清晰错误消息

### x666

**签到逻辑** (`src/services/checkin/providers/x666.rs`)
- POST `https://up.x666.me/api/checkin/spin` + Cookie
- 设置 Origin 和 Referer headers
- 优先检测"已签到"关键词（`今日已签`/`已签到`/`already`）
- JSON 解析失败时使用原始响应文本

**余额查询**
- GET `https://up.x666.me/api/checkin/status` + Cookie
- 读取 `current_quota` 字段（支持字符串/数字）

## 错误处理

所有 provider 统一的容错策略：

1. **JSON 解析失败** → 将原始响应文本作为消息使用
2. **HTTP 非 2xx** → 先检查是否"已签到"，再判定失败
3. **空消息** → 使用默认消息（anyrouter 空消息视为已签到）
4. **HTML 404** → 不再导致 500 错误，正确识别为失败

## 账号验证

创建/更新账号时的自动验证：

```rust
// anyrouter 必须提供
- userId: String (required)
- cookie: String (required)
- authType: "cookie" (自动设置)

// x666 必须提供
- cookie: String (required)
- authType: "cookie" (自动设置)

// new-api 可选认证
- authType: "access_token" → 必须提供 accessToken
- authType: "cookie" → 必须提供 cookie
```

## 反爬算法说明（anyrouter）

acw_sc__v2 求解流程：

```rust
1. 从 HTML 提取 `var arg1='[hex]'`
2. 按预定义索引表 ACW_SC_V2_INDEXES 重排序字符
3. 将重排后的 hex 与固定密钥 ACW_SC_V2_KEY 逐字节 XOR
4. 结果作为 Cookie: acw_sc__v2={result}
5. 使用新 Cookie 重新请求
```

索引表和密钥见 `anyrouter.rs` 常量定义。

## 余额数据格式

前端显示时将原始 quota 除以 500000 转换为 USD：

```typescript
const QUOTA_PER_USD = 500000;
const usd = quota / QUOTA_PER_USD;
return `余额 $${usd.toFixed(2)}`;
```

后端只负责存储和返回原始 quota 值。

## 调试建议

查看详细日志：

```bash
RUST_LOG=debug cargo run
```

关键日志搜索：
- `"Scheduled checkin completed"` - 调度执行成功
- `"Scheduled checkin failed"` - 调度执行失败
- `"Internal error: expected value"` - JSON 解析错误（v2.2.0 已修复）
- `"反爬挑战页"` - 检测到 anyrouter 反爬，检查 Cookie 是否有效

## 版本历史

- **v2.2.0** (2026-06-15) - 新增 anyrouter 余额查询、完善错误处理、账号验证
- **v2.1.0** (2026-06-13) - 安全加固、性能优化、时区修复
- **v2.0.0** (2026-06-12) - 用户管理、new-api/x666 余额查询
- **v1.0.0** (2026-06-10) - 初始版本
