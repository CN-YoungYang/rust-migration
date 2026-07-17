# Rust 文档索引

## 使用与部署

- [快速开始](QUICKSTART.md)：Docker、本地开发和首次启动。
- [操作指南](OPERATIONS.md)：账户、签到记录、统计、通知和全局设置。
- [管理员设置](ADMIN-SETUP.md)：初始管理员、密码和角色配置。
- [管理员功能](ADMIN-FEATURES.md)：权限边界、用户管理和管理员操作。
- [1Panel 部署](1PANEL-DEPLOY.md)：1Panel 环境部署与排查。
- [1C1G 优化](ORACLE-1C1G-OPTIMIZATION.md)：低资源服务器配置。
- [Next.js 迁移](MIGRATION.md)：历史数据库迁移与回滚。

## 其他入口

- [Rust 项目说明](../README.md)：功能、配置、开发和 API 概览。
- [版本记录](../CHANGELOG.md)：已完成变更和修复历史。
- [Provider 说明](../PROVIDERS.md)：站点适配和认证差异。
- [前端开发说明](../frontend/README.md)：Vue 开发与构建。

## 维护规则

- 当前行为以源码、`README.md`、`CHANGELOG.md`、`OPERATIONS.md` 和 `ADMIN-FEATURES.md` 为准。
- 认证模型统一为 Cookie 会话与 CSRF，不使用前端 Bearer token。
- 已完成事项写入 `CHANGELOG.md`，不再创建进度、自查或“优化完成”类重复文档。
- 可选优化统一维护在根目录 `docs/FURTHER-OPTIMIZATION-SUGGESTIONS.md`。
