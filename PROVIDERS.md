# 签到 Provider 实现说明

本文档说明各站点类型的签到和余额查询实现细节。

## 支持的站点类型

| 站点类型 | 签到端点 | 余额查询端点 | 认证方式 | 特殊处理 |
|---------|---------|------------|---------|---------|
| new-api | `/api/user/checkin` | `/api/user/self` | Bearer Token / Cookie | 7 个 compat userId 头 |
| arrouter | `/api/user/sign_in` | `/api/user/self` | Cookie + userId | acw_sc__v2 反爬（余额查询带 7 个 compat 头） |
| x666 | `/api/checkin/spin` | `/api/checkin/status` | Cookie | - |

> **userId 兼容头**（7 个，对齐 Next.js `COMPAT_USER_ID_HEADER_NAMES`）：
> `New-API-User` / `Veloera-User` / `X-Api-User` / `voapi-user` / `User-id` / `Rix-Api-User` / `neo-api-user`
> new-api 的签到与余额查询、arrouter 的余额查询都会带上全部 7 个；arrouter 的签到只发 `User-id`（与 Next.js 一致）。

## 实现细节

### new-api

**签到逻辑** (`src/services/checkin/providers/new_api.rs`)
- 发送 POST `{baseUrl}/api/user/checkin`
- **认证**：`access_token`（Bearer）与 `cookie` 都按账号实际配置传递，二者均可选（对齐 Next.js）
- **userId 头**：带上全部 7 个 compat 头
- 状态判定顺序：已签关键词 > `data.checked_in` / `checkedIn` 标志 > `success` 字段（对齐 Next.js）
- **本次获得额度**：解析 `data.quota_awarded` / `quotaAwarded` / `quota`，拼入消息「本次获得额度：xxx quota（约 $x.xx）」

**余额查询**
- GET `{baseUrl}/api/user/self`
- access_token（Bearer）+ cookie + 7 个 compat 头
- 解析路径：`quota` > `data.quota` > `data`（数字）> `balance` > `credit` > `amount`
- 支持字符串和数字类型

### arrouter

**签到逻辑** (`src/services/checkin/providers/arrouter.rs`)
- POST `{baseUrl}/api/user/sign_in` + Cookie + **User-id** 头（签到只发这一个，与 Next.js 一致）
- **反爬处理**：检测 HTML 中的 `acw_sc__v2` 和 `var arg1`
  - 提取 arg1 hex 字符串
  - 按索引表重排序并与密钥 XOR
  - 自动在 Cookie 中添加 `acw_sc__v2={解算值}` 重试
- 空消息判定为"已签到"（arrouter 特性）
- 支持 `success` 字段和消息关键词检测

**余额查询**
- GET `{baseUrl}/api/user/self` + Cookie + userId + **7 个 compat 头**
- ⚠️ `/api/user/self` 强制校验 `New-API-User`，必须带齐 compat 头，否则 401（v2.2.1 修复）
- 不传 access_token（与 Next.js 对齐）
- 同样支持反爬挑战自动重试（acw_sc__v2）
- 解析路径：`quota` > `data.quota` > `data`（数字）> `balance` > `credit` > `amount`
- 失败时返回清晰错误消息

### x666

**签到逻辑** (`src/services/checkin/providers/x666.rs`)
- POST `https://up.x666.me/api/checkin/spin` + Cookie
- 设置 Origin 和 Referer headers
- 优先检测"已签到"关键词（`今日已签`/`已签到`/`already`）
- JSON 解析失败时使用原始响应文本
- **本次获得额度**：解析 `data.quota` / `quota`，拼入消息「本次获得额度：xxx quota（约 $x.xx）」

**余额查询**
- GET `https://up.x666.me/api/checkin/status` + Cookie
- 解析路径：`current_quota` > `quota` > `data.current_quota` > `data.quota` > `balance` > `credit`（支持字符串/数字）

## 签到后刷新余额（v2.2.1）

`runner.rs` 的 `execute_checkin` 在签到状态为 `success` 或 `already_checked` 时，会自动调用 `fetch_account_balance` 更新账户余额（对齐 Next.js `runner.ts`）：

- 余额刷新成功 → 写入 `lastBalance` / `lastBalanceAt`
- 余额刷新失败 → **不影响签到结果**，仅在消息追加「余额刷新失败：xxx」
- 失败账号的签到结果仍按签到本身的状态记录

`fetch_account_balance` 按 site_type 分派：
- `x666` → 仅 cookie
- `arrouter` → userId + cookie（不传 access_token）
- new-api 及其他 → userId + access_token + cookie

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
- `"Scheduled checkin completed"` / `"Scheduled checkin failed"` - 调度执行结果
- `"Refreshing balance"` / `"Balance refreshed successfully"` - 余额刷新链路
- `"签到后余额刷新失败"` / `"签到后余额写库失败"` - 签到后刷新余额的失败（不影响签到）
- `"acw_sc__v2 求解失败"` - 反爬求解失败（arg1 长度不匹配，WAF 算法可能已升级）
- `"反爬挑战页"` - 检测到 arrouter 反爬，检查 Cookie 是否有效
- `"Balance field not found"` - 余额字段未找到，站点响应结构可能变化

## 版本历史

- **v2.2.1** (2026-06-15) - 修复余额查询（gzip/401/panic）、签到逻辑对齐 Next.js（cookie/awardedQuota/checked_in）、签到后刷新余额、Docker 多阶段构建
- **v2.2.0** (2026-06-15) - 新增 arrouter 余额查询、完善错误处理、账号验证
- **v2.1.0** (2026-06-13) - 安全加固、性能优化、时区修复
- **v2.0.0** (2026-06-12) - 用户管理、new-api/x666 余额查询
- **v1.0.0** (2026-06-10) - 初始版本
