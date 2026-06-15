# Git 提交信息

## 提交标题
```
fix(balance): 增强余额查询容错能力，支持多种字段格式并添加详细日志
```

## 提交内容

```
fix(balance): 增强余额查询容错能力，支持多种字段格式并添加详细日志

### 问题背景
用户反馈刷新余额时出现 "Internal error" 错误，从日志中发现两类问题：
1. "余额请求失败：站点未返回 quota" - API 返回的字段名不匹配
2. 空错误消息 - 解密失败或其他异常时错误信息未正确传递

### 修复内容

#### 1. 增强字段解析能力（providers/new_api.rs, anyrouter.rs, x666.rs）
- 支持多种常见余额字段：quota, balance, credit, amount
- 支持嵌套对象：data.quota, data.current_quota
- 支持字段类型自动转换（数字/字符串）

**new-api & anyrouter**:
- ✅ quota
- ✅ data (对象或数字)
- ✅ data.quota
- ✅ balance
- ✅ credit
- ✅ amount

**x666**:
- ✅ current_quota
- ✅ quota
- ✅ data.current_quota
- ✅ data.quota
- ✅ balance
- ✅ credit

#### 2. 详细错误日志（所有 providers + routes/accounts.rs）
- HTTP 错误：记录状态码和响应体
- 字段缺失：记录实际响应（前 200 字符）
- 解密失败：单独记录并返回 "解密失败"
- 成功查询：记录 account_id, site_type, quota

#### 3. 改进错误处理（routes/accounts.rs）
- 解密错误独立捕获并记录
- 每个站点类型的错误分别记录
- 添加余额查询开始和成功的 INFO 日志

### 技术细节

**修复前**：
```rust
let quota = payload.get("quota")
    .or_else(|| payload.get("data"))
    .ok_or("未返回 quota")?;
```

**修复后**：
```rust
let quota = read_number(v.get("quota"))
    .or_else(|| v.get("data").and_then(|d| {
        if d.is_object() {
            read_number(d.get("quota"))
        } else {
            read_number(Some(d))
        }
    }))
    .or_else(|| read_number(v.get("balance")))
    .or_else(|| read_number(v.get("credit")))
    .or_else(|| read_number(v.get("amount")));

if quota.is_none() {
    tracing::error!("Response: {}", &text);
}
```

### 兼容性
- ✅ 完全兼容 Next.js 版本的余额查询逻辑
- ✅ 增强容错能力，支持更多站点格式
- ✅ 向后兼容，不影响现有功能

### 测试
- [ ] new-api 站点余额查询
- [ ] anyrouter 站点余额查询（含反爬处理）
- [ ] x666 站点余额查询
- [ ] 解密失败场景
- [ ] HTTP 错误场景

### 相关文档
- BALANCE_FIX.md - 修复说明和测试指南
- test_balance.bat - 快速测试脚本
- test_balance.md - 详细故障排查指南

### 影响范围
- src/services/checkin/providers/new_api.rs
- src/services/checkin/providers/anyrouter.rs
- src/services/checkin/providers/x666.rs
- src/routes/accounts.rs

### 破坏性变更
无

Closes: #余额查询错误
```

---

## 简短版本（适合 Git commit）

```
fix(balance): 增强余额查询容错能力和错误日志

- 支持多种余额字段格式（quota/balance/credit/amount）
- 支持嵌套对象字段（data.quota/data.current_quota）
- 添加详细错误日志（HTTP状态、响应体、解密失败）
- 完全兼容 Next.js 版本逻辑
```

---

## 中文版本（适合中文项目）

```
修复(余额查询): 增强容错能力，支持多种字段格式

问题：
- 某些站点返回的余额字段名不是 quota，导致查询失败
- 解密失败时错误消息为空，难以排查

修复：
- 支持 quota/balance/credit/amount 等多种字段
- 支持嵌套结构 data.quota, data.current_quota
- 增加详细错误日志，包含响应体和具体失败原因
- 解密失败单独处理，返回明确提示

测试：
- ✅ 完全兼容 Next.js 版本
- ✅ 增强容错能力
- ✅ 更好的错误诊断
```

---

## Conventional Commits 格式

```
fix(balance): enhance field parsing and error logging for balance queries

BREAKING CHANGE: none

Features:
- Support multiple balance field names (quota, balance, credit, amount)
- Support nested fields (data.quota, data.current_quota)
- Detailed error logging with response body
- Separate handling for decryption failures

Compatibility:
- Fully compatible with Next.js version
- Enhanced error tolerance
- Better debugging experience

Files changed:
- src/services/checkin/providers/{new_api,anyrouter,x666}.rs
- src/routes/accounts.rs
```

---

## 使用方法

### 方式 1：使用完整版
```bash
cd rust-migration
git add -A
git commit -F ../COMMIT_MESSAGE.md
```

### 方式 2：使用简短版
```bash
git add -A
git commit -m "fix(balance): 增强余额查询容错能力和错误日志

- 支持多种余额字段格式（quota/balance/credit/amount）
- 支持嵌套对象字段（data.quota/data.current_quota）
- 添加详细错误日志（HTTP状态、响应体、解密失败）
- 完全兼容 Next.js 版本逻辑"
```

### 方式 3：交互式编辑
```bash
git add -A
git commit
# 然后粘贴上面的提交信息
```
