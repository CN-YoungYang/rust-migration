# 更新日志

## v2.0.0 (2026-06-12)

### ✨ 新功能

#### 余额查询
- 支持 new-api 和 x666 站点余额实时查询
- 前端每个账号添加「查询余额」按钮
- 自动更新 `lastBalance` 和 `lastBalanceAt` 字段

#### 记录清理
- 管理员可清理签到记录，保留最新 N 条（默认100）
- 普通用户只能清理自己账号的记录
- 管理员清理所有用户的记录

#### 权限控制系统
- **���通用户（USER）**：
  - 只能查看和操作自己创建的账号
  - 只能查看自己账号的签到记录
  - 清理记录时仅删除自己的

- **管理员（ADMIN/SUPER_ADMIN）**：
  - 查看所有账号，支持用户筛选
  - 可筛选：所有用户 / 仅我的 / 指定用户
  - 操作所有账号和记录
  - 清理时删除所有用户的记录

### 🔧 改进

#### 后端
- 账号表添加 `ownerId` 字段关联创建者
- 所有账号操作添加所有权检查
- 签到记录查询支持按用户过滤
- 新增数据库函数：
  - `list_accounts_by_user` - 按用户查询账号
  - `list_runs_by_user` - 按用户查询记录
  - `cleanup_checkin_runs_by_user` - 按用户清理记录
  - `update_account_balance` - 更新账号余额

#### 前端
- 账号列表添加用户筛选下拉框（仅管理员可见）
- 每个账号添加「查询余额」按钮
- 签到记录页添加「清理记录」按钮（管理员可见）
- 支持 SUPER_ADMIN 角色显示

### 🗄️ 数据库变更

```sql
-- 添加 ownerId 字段
ALTER TABLE CheckinAccount ADD COLUMN ownerId TEXT;

-- 为现有账号设置所有者（设为第一个管理员）
UPDATE CheckinAccount SET ownerId = (SELECT id FROM AppUser ORDER BY createdAt LIMIT 1) WHERE ownerId IS NULL;
```

### 📡 API 变更

#### 新增接口
- `POST /api/accounts/:id/refresh-balance` - 查询账号余额
- `DELETE /api/checkin-runs/cleanup` - 清理签到记录

#### 修改接口
- `GET /api/accounts?userId=xxx` - 支持用户筛选参数（仅管理员）
- 所有账号操作接口添加权限检查

### ⚠️ 破坏性变更

- 所有账号操作现在需要权限验证
- 普通用户无法访问其他用户的账号
- 需要执行数据库迁移添加 `ownerId` 字段

### 🔄 迁移步骤

1. **备份数据库**：
   ```bash
   cp data/app.db data/app.db.backup
   ```

2. **停止旧服务**

3. **更新代码并构建**：
   ```bash
   cd frontend
   npm install
   npm run build
   cd ..
   cargo build --release
   ```

4. **启动新服务**（自动执行迁移）：
   ```bash
   ./target/release/ai-hub-rust
   ```

5. **验证迁移**：
   - 检查所有账号是否有 `ownerId`
   - 测试普通用户只能看到自己的账号
   - 测试管理员可以切换用户筛选

---

## v1.0.0 (2026-06-10)

初始 Rust 版本发布