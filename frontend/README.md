# AI Hub Rust 前端

这是 `rust-migration/` 的 Vue 3 + Vite + TypeScript 前端。界面采用中文工作台布局和原生 CSS 动效，并保持低资源部署目标。

## 功能页面

- `App.vue`：登录、会话检查、响应式侧边导航和在线状态。
- `AccountPanel.vue`：账户管理、筛选、分组、多选、批量操作、CSV 导入导出。
- `CheckinRunsPanel.vue`：签到记录、筛选、失败重试、摘要复制、记录清理。
- `StatisticsPanel.vue`：统计概览、趋势、站点统计、最近失败。
- `NotificationPanel.vue`：通知配置、校验、预览、测试结果。
- `SettingsPanel.vue`：全局调度和清理设置。
- `AdminUserPanel.vue`：管理员用户管理和账户摘要。

## 开发

```bash
npm install
npm run dev
```

开发服务器默认 `http://localhost:5173`，并把 `/api` 代理到 `http://localhost:8080`。开发时需要同时启动 Rust 后端：

```bash
cd ..
cargo run
```

## 构建

```bash
npm run build
```

构建产物输出到 `../public/`，由 Axum 在生产环境中提供静态文件服务。

## 视觉与动效约定

- 全局设计令牌位于 `tokens.css`，基础规则位于 `src/style.css`，工作台规则位于 `src/workbench.css`。
- 动效仅使用短时 CSS 过渡，不引入 JavaScript 动画运行时。
- `prefers-reduced-motion: reduce` 时关闭非必要动效，并保留完整交互。

## API 约定

所有请求应使用 `src/utils/api.ts`：

- 自动携带 Cookie：`credentials: 'include'`。
- 对 `POST` / `PUT` / `DELETE` / `PATCH` 自动注入 `X-CSRF-Token`。
- 统一解析 `{ data: T }` 和 `{ error: string }`。
- 非登录探测接口返回 401 时触发会话过期事件。

## 组件约定

- 组件文件使用 `PascalCase`。
- 复杂共享状态放到 `src/composables/`。
- 异步操作必须有 loading 状态，按钮应防重复提交。
- 定时器和事件监听器必须在 `onUnmounted` 清理。
- 可选字段清空时使用 `null`，字段不变时省略字段。
