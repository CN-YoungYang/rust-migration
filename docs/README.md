# 文档索引

欢迎查阅 AI Hub Rust 版本文档!

## 🚀 快速上手

1. **[QUICKSTART.md](QUICKSTART.md)** - 5分钟快速开始
   - Docker Compose 部署
   - 本地开发
   - 验证功能

2. **[1PANEL-DEPLOY.md](1PANEL-DEPLOY.md)** - 1Panel 图形界面部署
   - 分步截图指导
   - 配置说明
   - 常见问题

3. **[MIGRATION.md](MIGRATION.md)** - 从 Next.js 版本迁移
   - 数据库兼容性
   - 迁移步骤
   - 回滚方案

## 🔑 管理员指南

4. **[ADMIN-SETUP.md](ADMIN-SETUP.md)** - 管理员账号设置
   - 默认账号
   - 自定义配置
   - 密码修改
   - 安全建议

5. **[ADMIN-FEATURES.md](ADMIN-FEATURES.md)** - 管理员功能详解
   - 权限说明
   - API 使用示例
   - 常见操作

6. **[OPERATIONS.md](OPERATIONS.md)** - 当前操作指南
   - 账户批量操作
   - 签到记录与失败重试
   - 统计、通知、全局设置

## ⚙️ 优化指南

7. **[ORACLE-1C1G-OPTIMIZATION.md](ORACLE-1C1G-OPTIMIZATION.md)** - 甲骨文 1C1G 优化
   - 低配服务器优化
   - Docker 资源限制
   - 性能调优
   - 监控方案

8. **[OPTIMIZATION-CHECKLIST.md](OPTIMIZATION-CHECKLIST.md)** - 优化清单
   - 已实现优化
   - 可选优化
   - 性能基准
   - 场景适配

## 📝 技术文档

9. **[PROGRESS.md](PROGRESS.md)** - 开发进度记录
   - 开发历程
   - 任务清单
   - 里程碑

10. **[SELF-CHECK-REPORT.md](SELF-CHECK-REPORT.md)** - 自查报告
    - 代码审查
    - 问题发现
    - 修复记录

## 📊 文档分类

### 新手必读
1. QUICKSTART.md
2. ADMIN-SETUP.md
3. OPERATIONS.md

### 1Panel 用户
1. 1PANEL-DEPLOY.md
2. ORACLE-1C1G-OPTIMIZATION.md

### 从 Next.js 迁移
1. MIGRATION.md
2. ADMIN-FEATURES.md

### 性能优化
1. ORACLE-1C1G-OPTIMIZATION.md
2. OPTIMIZATION-CHECKLIST.md

### 技术深入
1. PROGRESS.md
2. SELF-CHECK-REPORT.md
3. OPTIMIZATION-CHECKLIST.md

## 🔍 快速查找

- **如何部署?** → QUICKSTART.md 或 1PANEL-DEPLOY.md
- **管理员账号?** → ADMIN-SETUP.md
- **管理员能做什么?** → ADMIN-FEATURES.md
- **日常怎么操作?** → OPERATIONS.md
- **1C1G 服务器优化?** → ORACLE-1C1G-OPTIMIZATION.md
- **从 Next.js 迁移?** → MIGRATION.md
- **性能怎么样?** → OPTIMIZATION-CHECKLIST.md
- **有哪些历史进展?** → PROGRESS.md

## 👍 推荐阅读顺序

### 初次使用
1. QUICKSTART.md (快速部署)
2. ADMIN-SETUP.md (设置管理员)
3. OPERATIONS.md (日常操作)
4. ADMIN-FEATURES.md (了解权限)

### 1Panel 用户
1. 1PANEL-DEPLOY.md (图形界面部署)
2. ORACLE-1C1G-OPTIMIZATION.md (性能优化)
3. ADMIN-FEATURES.md (使用指南)

### 技术研究
1. PROGRESS.md (开发历程)
2. SELF-CHECK-REPORT.md (自查结果)
3. OPTIMIZATION-CHECKLIST.md (优化细节)

## 文档维护规则

- 当前功能说明优先看 `README.md`、`CHANGELOG.md`、`OPERATIONS.md`、`ADMIN-FEATURES.md`。
- 历史进度、自查和优化记录仅用于追溯，不作为接口或操作的唯一依据。
- 认证模型以 Cookie 会话 + CSRF 为准，不再使用前端保存 Bearer token。
- 旧的 `FIXES-SUMMARY.md` 已移除，其内容被 `CHANGELOG.md` 和本目录当前指南取代。
- 旧的 `FRONTEND-INTEGRATION.md` 已移除，前端入口改看 `frontend/README.md` 和 `vite.config.ts`。

---

**提示**: 所有文档均为 Markdown 格式,可以在 GitHub 、VS Code 或任何 Markdown 阅读器中查看。
