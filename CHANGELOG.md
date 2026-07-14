# 更新日志

## 未发布

### 安全

- **自定义签到 URL 同源限制**：anyrouter/x666 仅允许相对路径或与账户 `baseUrl` 协议、主机、端口完全一致的绝对 URL；创建、更新、CSV 导入和实际执行前统一校验，阻止管理员修改 URL 后把已有 Cookie 发往第三方地址。

### 可靠性

- 通知类型异常与 SMTP STARTTLS 状态异常现在返回统一 `AppError`，不再依赖 `unreachable!` 触发进程 panic。

### 修复

- 签到历史清理现在遵循管理员当前用户筛选范围，避免筛选单个用户后误删全局记录。
- 清空全部历史时可原子重置账户最近签到状态与失败计数，同时保留余额信息。

- x666 自定义相对签到路径现在会正确拼接到 `baseUrl`，不再作为无效相对请求直接发送。
- new-api 账户表单不再展示或提交未被 Provider 使用的自定义签到 URL。

### 界面

- 登录页优化字体、间距、表单层次和导航状态；退出按钮改用明确选择器，移除 `!important` 优先级覆盖。
- 新增“跳到主要内容”键盘入口，并让主内容区域可接收程序性焦点。

- 签到记录清理控件明确展示操作范围，清空全部时提供状态重置选项，并反馈实际删除与重置数量。

- 账户表单按站点类型和认证方式显示必要字段：new-api 显示对应 Token/Cookie，anyrouter 显示 userId/Cookie/自定义路径，x666 仅显示 Cookie/自定义路径。
- 自定义签到 URL 字段增加同源安全提示。

### 测试

- 新增未知通知类型校验测试。

- 新增签到历史清理权限范围、原子状态重置和前端请求载荷测试。

- 新增 6 个 Rust 同源 URL 测试和 4 个前端 Provider 字段矩阵测试。

## v2.7.0 (2026-07-09)

### 安全

- **CSRF 比较改为常量时间**：`auth_middleware` 中 CSRF token 校验由短路 `==` 改为自实现常量时间比较，避免基于响应时间的旁路猜测。
- **SMTP 通知 SSRF 发送时复核**：`send_smtp` 发送邮件前再次按实际解析 IP 做内网/私网校验，与 webhook 路径对齐，堵住配置写入到通知发送之间的 DNS 重绑定/TOCTOU 缺口。
- **会话滑动续期**：`find_session_and_renew` 命中且未过期时，若距过期已不足 TTL 一半则更新 `lastSeenAt` 并顺延 `expiresAt`，活跃用户不掉线；距过期仍远则直接返回，避免每请求写库。会话语义由固定 TTL 变为滑动 TTL。

### 数据一致性

- **签到余额写入并入事务**：新增 `create_run_with_status_update_and_balance`，将账户状态更新、余额刷新（成功时）与签到记录创建放在同一事务原子提交，修复此前进程崩溃可能出现的“余额已更新但无签到记录”部分写入。余额刷新为网络请求仍前置执行，但写库与状态/记录一并提交。

### 测试


- 新增 CSRF 常量时间比较、会话续期阈值、签到余额事务写入共 5 个单元测试。

### 界面与可访问性

- **统一设计 token**：在全局 `style.css` 引入 `:root` 设计 token（背景/边框/文本/主色/语义/圆角/阴影），App.vue 与全部六个面板的 scoped 样式由硬编码十六进制迁移为引用 token，颜色与原值逐位等价，便于后续统一调主题。
- **可访问性**：导航 `aria-label`、功能切换 `role="tab"`/`aria-selected`、离线与服务器状态条 `role="status"`/`aria-live`、状态点 `aria-hidden`；全局新增 `prefers-reduced-motion` 降级；各面板补 `focus-visible` 键盘聚焦环。
- **响应式**：窄屏下导航横向滚动、筛选/表单行单列堆叠、统计与概览网格收为单列、签到记录失败项按钮全宽、通知配置表单堆叠。
- 新增共享 `.empty-state` / `.loading-state` 占位样式，供面板复用。

## v2.6.0 (2026-06-29)

### 界面与操作体验

- **账户管理全面增强**
  - 支持账户多选、选中账户批量签到、批量刷新余额、批量启用、批量禁用。
  - 批量操作增加进度条、当前处理账户、失败摘要和批量签到结果明细。
  - 账户卡片显示启用状态、最近签到、余额刷新时间、今日执行次数、归属用户和备注。
  - 筛选支持站点类型、启用状态、签到状态、关键词；关键词覆盖账户名称、站点地址和备注。
  - 编辑账户时锁定站点类型和认证方式，避免破坏既有凭证语义。
  - 编辑时空 `userId`、`customCheckinUrl`、`note` 会以 `null` 提交并清空数据库字段。

- **签到记录页增强**
  - 支持失败账户一键批量重试、单条失败记录重试。
  - 增加当前加载记录概览、成功/失败/进行中计数和平均耗时。
  - 支持复制单条签到摘要，便于排查 provider 响应或反馈问题。
  - 记录清理保留数量支持 `0`，表示清空全部，并保留后端边界校验。

- **数据统计页增强**
  - `GET /api/statistics` 新增 `recentFailures` 字段。
  - 前端新增最近失败列表、风险站点摘要、今日执行概览、7/30/90 天快捷筛选。
  - 最近失败支持复制摘要，管理员筛选指定用户时仍保持 ownerId 权限边界。

- **通知配置页增强**
  - 支持 email、webhook、telegram 配置的本地校验、触发条件预览、发送目标预览。
  - 测试通知结果在界面留痕，便于连续调试。
  - 更新通知时支持通过 `null` 清空 `balanceThreshold`、`webhookHeaders`、`note`。

- **全局设置页增强**
  - 暴露 `cleanupKeepLatest` 配置，控制定时清理保留的最新签到记录数。
  - 增加保存前校验和当前执行策略预览。
  - `batchDelayMin` / `batchDelayMax` 允许合法 `0` 值，用于关闭批量/定时签到账户间等待。

- **用户管理页增强**
  - 用户列表显示账户总数、启用账户数、失败账户数、最近签到时间。
  - 创建和保存用户时增加 loading 状态，避免重复提交。

### 后端与数据层

- **设置更新字段映射修复**
  - `UpdateSettingsRequest` 支持前端 camelCase 字段：`windowStart`、`windowEnd`、`retryEnabled`、`maxAttemptsPerDay`、`batchDelayMin`、`batchDelayMax`、`cleanupKeepLatest`。
  - 保留 snake_case alias，兼容脚本或旧调用。

- **设置 0 值语义修复**
  - `get_settings()` 不再把合法的 `batchDelayMin=0`、`batchDelayMax=0`、`cleanupKeepLatest=0` 改回默认值。
  - 旧数据库补列默认值调整为 `batchDelayMin=3`、`batchDelayMax=10`，与新库迁移保持一致。

- **三态字段更新**
  - `UpdateAccountRequest` 支持缺失 / `null` / 有值三态，允许清空可选账户字段。
  - `UpdateNotificationRequest` 支持缺失 / `null` / 有值三态，允许清空可选通知字段。

- **账户、统计和用户聚合**
  - 账户关键词筛选增加备注字段。
  - 管理员用户列表返回账户摘要统计。
  - 统计最近失败按 owner 过滤并按时间倒序返回。

### 前端通用改进

- 统一 401 会话过期事件，非登录探测接口返回 401 时自动提示重新登录。
- 登录成功后直接使用登录响应中的用户信息，避免二次 `/auth/me` 失败造成状态错乱。
- 首屏增加认证检查状态，登录按钮防重复提交。
- 顶部显示当前用户和角色。
- Toast 限制最大数量，移动端展示更稳定；确认框支持 Esc / Enter。

### 数据库

- `CheckinSetting` 新增 `cleanupKeepLatest INTEGER NOT NULL DEFAULT 500`。
- 运行时兼容旧库，缺失列会通过 `ensure_setting_columns()` 幂等补齐。

### 测试与验证

- 新增账户三态更新、通知三态更新、设置字段映射、设置 0 值保留、用户账户摘要、统计最近失败等测试。
- 验证命令：
  - `cargo fmt -- --check`
  - `cargo test`（26 passed）
  - `cargo clippy -- -D warnings`
  - `npm run build`
  - `git diff --check`
- 真实临时 SQLite + HTTP 流程验证通过：健康检查、登录、CSRF、`auth/me`、设置保存与非法值拦截、统计、通知清空、账户备注筛选、账户字段清空、退出。

---

## v2.5.0 (2026-06-25)

### 新功能

- **批量导入/导出账户** — 支持 CSV 格式的账户批量管理
  - **导出功能**：一键导出所有账户为 CSV 文件，包含完整的加密凭证（自动解密）
  - **导入功能**：支持 CSV 文件批量导入账户，自动加密敏感信息
  - **详细反馈**：导入时显示成功/失败统计，错误详情可展开查看
  - **格式说明**：内置 CSV 格式说明和示例，方便用户理解字段要求
  - **数据隔离**：普通用户只能导出自己的账户，管理员可导出全局数据

### 后端改进

- 新增 `GET /api/accounts/export` 导出 API
  - 自动解密所有敏感字段（accessToken, cookie）
  - 生成标准 CSV 格式，包含时间戳的文件名
  - 支持按用户权限过滤导出数据
- 新增 `POST /api/accounts/import` 导入 API
  - 逐行解析 CSV，自动验证必填字段
  - 自动加密敏感凭证后存储
  - 返回详细的导入结果（成功数、失败数、错误信息）
- 新增 `csv::Error` 到 `AppError` 的转换支持

### 前端改进

- 账户管理页面新增"导出 CSV"和"导入 CSV"按钮
- 导入对话框包含：
  - 文件选择器（仅接受 .csv 文件）
  - CSV 格式说明和示例（可折叠）
  - 实时导入进度和结果展示
  - 错误列表（可折叠查看详情）
- 导出自动触发浏览器下载，文件名包含日期时间
- 导入成功后自动刷新账户列表

### CSV 格式

**必填字段**：name, siteType, baseUrl, authType
**可选字段**：userId, accessToken, cookie, customCheckinUrl, enabled, retryEnabled, note

示例：
```csv
name,siteType,baseUrl,authType,accessToken,cookie,enabled
测试账户,new-api,https://api.example.com,access_token,sk-xxx,,true
```

---

## v2.4.0 (2026-06-25)

### 新功能

- **数据统计面板** — 新增完整的数据可视化功能，帮助全面了解系统运行状况
  - **概览卡片**：总账户数、今日成功数、总成功率、总余额等关键指标一览
  - **每日签到趋势图**：可视化展示每日签到成功/失败/已签到的分布，支持自定义时间范围（默认最近30天）
  - **站点统计表格**：按站点类型汇总账户数、签到次数、成功率、平均响应时间等指标
  - **余额信息**：实时显示所有已启用账户的总余额（美元 + quota）
  - **数据隔离**：普通用户仅能查看自己的统计数据，管理员可查看全局数据

### 后端改进

- 新增 `GET /api/statistics` 统计 API，支持按时间范围查询（`startDate` / `endDate` 参数）
- 统计数据自动按用户权限过滤（`ownerId`），保障数据安全
- 优化 SQL 查询性能，使用 `JOIN` 和 `GROUP BY` 减少数据库往返次数

### 前端改进

- 新增"数据统计"导航入口，所有用户可见
- 响应式布局，自适应不同屏幕尺寸
- 支持日期范围选择器，方便查看特定时间段的数据
- 条形图可视化，直观展示每日签到趋势
- 色彩编码：绿色（成功）、橙色（已签到）、红色（失败）

---

## v2.3.4 (2026-06-25)

### 新功能

- **新增"今日未签到"筛选选项** — 账户管理页面新增 `not_today` 筛选状态，快速定位今天还没签到的账户，方便转点后批量操作。筛选逻辑：`lastRunAt IS NULL` 或 `DATE(lastRunAt, 'localtime') < DATE('now', 'localtime')`，涵盖从未签到和今天尚未签到两种情况

### 改进

- **优化筛选选项语义** — 将原 "未签到" 重命名为 "从未签到"（`never`），与新增的 "今日未签到"（`not_today`）形成区分，语义更清晰

---

## v2.3.1 (2026-06-16)

### Bug 修复

- **修复余额查询被 Cloudflare WAF 拦截（403）** — `zxiaoruan.cn` 等 Cloudflare 防护站点签到后余额刷新返回 `403 Sorry, you have been blocked`，导致余额写库失败。根因三层：① `fetch_balance` 之前完全不传 UA，永远用 `http_client` 单例的固定 UA；② UA 池停在 Chrome/126（2024-06），落后近两年，是 WAF 强 bot 信号；③ 缺 `Accept-Language` / `sec-ch-ua` / `Sec-Fetch-*` 等浏览器一致性头，“声称是 Chrome 但缺客户端提示”直接被扣分
- **修复旧库启动崩溃** — `CheckinSetting.batchDelayMin/Max` 两列在 v2.3.0 才引入。旧库执行 `20260611_init.sql` 时 `CREATE TABLE IF NOT EXISTS` 命中已存在的表直接跳过、不补列，随后 seed `INSERT` 显式引用这两列，触发 `table CheckinSetting has no column named batchDelayMin`，程序在启动迁移阶段直接崩溃（`ensure_setting_columns()` 在 `get_settings()` 才调用，崩溃时根本走不到）。修复：seed `INSERT` 不再写入这两列，新库由列定义 `DEFAULT 3/10` 取值，旧库由 `ensure_setting_columns()` 运行时补列并修正默认值，新老库均能正常启动

### 防判定（header / UA 层）

- **浏览器指纹池** — `checkin/mod.rs` 新增 `BrowserProfile`（UA + `sec-ch-ua` + `sec-ch-ua-platform` + `Accept-Language` 自洽组合），5 个近期版本指纹池：Chrome 134/135（Windows）、Chrome 135（macOS）、Firefox 137（Windows，无 `sec-ch-ua`）、Edge 135（Windows）
- **`random_browser_profile()` / `apply_browser_headers()`** — 每次签到 / 余额查询随机选一个 profile，统一注入 `User-Agent`、`Accept-Language`、`sec-ch-ua`、`sec-ch-ua-mobile`、`sec-ch-ua-platform`；`http_client` 单例默认 UA 同步升到 Chrome/135
- **三 provider 签名统一** — `checkin` / `fetch_balance` 由 `user_agent: Option<&str>` 改为 `profile: &BrowserProfile`，所有请求补齐 `Sec-Fetch-Site/Mode/Dest`、`Referer`（同源）；余额查询现在也走随机 UA，且与签到指纹保持一致

> 仅解决 header/UA 层拦截。若部署后仍 403，则拦截在更底层：JA3/JA4 TLS 指纹（需换 rrequest/BoringSSL 模拟 Chrome 握手）或 IP 信誉（Oracle Cloud 数据中心 IP 常被降权，需住宅代理）。

---

## v2.3.0 (2026-06-16)

### 新功能

- **账户 / 记录按归属用户分组** — 账户管理、签到记录均按 owner 折叠分组，避免多用户混淆；当前用户自己的分组置顶。账户 JSON 新增 `ownerId` / `ownerName`，`list` 路由构建 `owner_map` 一次性反查归属用户名
- **批量手动签到** — 新增 `POST /api/checkin-runs/batch` 端点（`main.rs` 注册）。支持「全部签到」/「该组签到」，自动跳过今日已 `success`/`already_checked`、已禁用、关闭重试、达到每日上限的账户；响应 `BatchCheckinResponse { items, total, succeeded, skipped, failed }`
- **非管理员隐藏全局设置** — 「全局设置」入口与 `SettingsPanel` 对 `role != ADMIN/SUPER_ADMIN` 隐藏（`App.vue` 用 `v-if="isAdmin"` 守卫），避免普通用户误改调度参数

### 防批量判定（批量 + 定时两条路径统一应用）

> 同一站点多账户瞬时并发是最大的机器人指纹。改为逐个签到，相邻账户之间按管理员设置随机延迟，并打乱执行顺序、轮换 UA。

- **由并发改为串行执行** — 账户间在 `[batchDelayMin, batchDelayMax]` 秒区间内随机 `sleep`；每轮执行前用 `SliceRandom::shuffle` 随机打乱账户顺序
- **随机 UA** — 每个账户签到使用随机 UA（v2.3.1 进一步升级为完整浏览器指纹 profile）
- **随机延迟可配置** — `CheckinSetting` / `UpdateSettingsRequest` 新增 `batchDelayMin` / `batchDelayMax`（默认 3 / 10 秒）；`settings.rs` 校验 `0 <= min <= max <= 600` 且二者须同时提供
- **调度器重构** — `scheduler.rs` 移除 `Semaphore(10)` 并发，改为串行执行 + 随机延迟 + 打乱顺序；跳过判断提取为 `runner::skip_reason_for_batch`，定时与批量手动签到共用同一套规则
- **db.rs 幂等迁移** — `ensure_setting_columns()` 对旧库用 `ALTER TABLE ADD COLUMN`（忽略 "duplicate column" 错误）补 `batchDelayMin/Max` 两列

### 构建 / 部署

- `Cargo.toml` 增加 `rand = "0.8"`
- 修复 `runner.rs` 字节级调用错误：`execute_anyrouter_checkin` 原误调 4-arg 的 x666 provider，现已正确命中 arrouter provider
- 前端重新构建，新 bundle `index-CzpkatJQ.js` / `index-BamDdJe3.css`

---

## v2.2.1 (2026-06-15)

### Bug 修复

- **修复余额查询响应乱码** — reqwest 开启 `gzip`/`brotli`/`deflate` 特性后自动解压响应体，解决站点返回 gzip 压缩数据导致 `serde_json` 解析失败、报"站点未返回 quota"的问题
- **修复 UTF-8 字节切片 panic** — new_api / arrouter / x666 三个 provider 中 `&text[..200]` 改为 `text.chars().take(200).collect()`，避免在多字节字符（中文）中间切断导致线程崩溃
- **修复前端余额显示错误** — `AccountPanel.vue` 的 `formatBalance` 之前直接把 `quota` 当美元，现按 One API 标准 `quota / 500000 = USD` 换算，与 Next.js 版本（`QUOTA_PER_USD = 500000`）完全对齐
- **修复 arrouter 余额查询 401** — `/api/user/self` 接口强制校验 `New-API-User` 头，但 `fetch_balance` 之前只发 `User-id` 一个头。现补全 7 个 compat 头（New-API-User / Veloera-User / X-Api-User / voapi-user / User-id / Rix-Api-User / neo-api-user），主请求与 acw_sc__v2 重试请求均已补齐

### 签到逻辑对齐 Next.js

- **new-api 签到补 cookie 认证** — `checkin` 签名由 `(base_url, token, user_id)` 改为 `(base_url, user_id, access_token, cookie)`。之前只传 access_token，cookie-only 站点签到会失败；现 access_token 与 cookie 都按实际配置传递
- **new-api 签到补 7 个 compat userId 头** — 与 fetch_balance 一致（之前签到路径完全没带这些头）
- **签到展示本次获得额度** — new-api 解析 `data.quota_awarded` / `quotaAwarded` / `quota`，x666 解析 `data.quota`，拼入消息「本次获得额度：xxx quota（约 $x.xx）」。新增共享 `format_awarded_quota`（QUOTA_PER_USD = 500000）
- **new-api checked_in 标志位判定** — 读 `data.checked_in` / `checkedIn` 布尔值判定今日已签（之前只靠 message 文本匹配，会漏判）
- **补全已签关键词** — `已签` → `已签` / `已经签到` / `今天已经签到`（对齐 Next.js）
- **签到成功后自动刷新余额** — 状态为 `success` 或 `already_checked` 时调用 `fetch_account_balance` 更新余额；余额刷新失败**不影响签到结果**，仅在消息追加「余额刷新失败：xxx」（完全对齐 Next.js runner.ts）

### 增强

- **反爬求解诊断日志** — `solve_acw_sc_v2` 各失败点（arg1 长度不匹配、非 hex 字符）及签到/余额查询两处调用点添加 `warn` 日志，便于将来 WAF 算法升级时快速定位（含 arg1 实际长度、期望长度、预览）
- **refresh_balance 全链路追踪** — `routes/accounts.rs` 在解密失败、provider 调用错误、成功各阶段添加 `info`/`warn`/`error` 日志

### 清理

- 删除误产生的 3 字节垃圾文件 `arrouter.rs`

### 构建 / 部署

- **Dockerfile 改为多阶段构建** — 新增 `frontend-builder`（node:22-slim）阶段，`docker build` 时自动 `npm ci && npm run build`，前端产物由 Stage 1 编译后 `COPY` 进运行镜像。**镜像内前端永远是源码最新编译结果，不再依赖宿主机 `public/`**（修复"改了前端但镜像里 index 不更新"的根因）
- **`.dockerignore` 调整** — 由排除整个 `frontend/` 改为仅排除 `frontend/node_modules/` 等缓存目录，让源码能进入构建上下文

---

## v2.2.0 (2026-06-15)

### 签到逻辑完善（参考 React 项目对齐）

- **x666 签到增强**
  - JSON 解析容错：处理 HTML 404 页面和非 JSON 响应
  - 优先检测"已签到"状态，避免误判
  - 优化错误消息提取逻辑（message/error 字段）
  - 余额查询支持字符串和数字类型的 current_quota

- **anyrouter 签到增强**
  - 成功消息检测支持英文 "success" 和中文 "签到成功"
  - JSON 解析失败时使用原始响应文本（不再返回空消息）
  - 保持 acw_sc__v2 反爬验证码自动求解逻辑

- **anyrouter 余额查询新增** 🎉
  - 实现 `fetch_balance()` 函数，使用 `/api/user/self` 端点
  - 自动处理反爬挑战页面（acw_sc__v2 自动重试）
  - 支持从 `quota` 或 `data` 字段提取余额
  - refresh_balance API 现在支持三种账号类型（new-api/anyrouter/x666）

### 账号管理增强

- **输入验证逻辑**
  - anyrouter 账号必须提供 userId 和 cookie
  - x666 账号必须提供 cookie
  - new-api 使用 access_token 认证时必须提供 accessToken
  - 更新账号时防止清除必需的凭证字段

- **authType 自动调整**
  - 创建 anyrouter 或 x666 账号时自动设置 authType 为 "cookie"
  - 避免用户手动配置错误

### 错误处理改进

- 修复：`expected value at line 1 column 1` 导致的 500 错误
- 修复：401/404 等 HTTP 错误返回的错误消息正确提取和显示
- 增强：所有 provider 统一的错误处理模式

---

## v2.1.0 (2026-06-13)

### 安全加固

- **登录时序攻击防护** — 用户不存在时也执行 dummy bcrypt 校验，消除用户名枚举风险
- **登录频率限制统一** — 所有登录失败（用户不存在、密码错误、账户禁用）均记录频率
- **设置接口权限修复** — GET/PUT `/api/settings` 添加 admin 角色校验，之前任何登录用户都能修改全局设置
- **错误信息隔离** — 内部错误（数据库、加密）不再向客户端泄露详情，仅在服务端日志记录
- **所有权校验** — 所有账户操作（查看、编辑、删除、刷新余额）检查 ownerId

### 功能修复

- **全局重试开关生效** — 调度器现在同时检查全局 `retryEnabled` 和账户级别设置（之前全局开关无效）
- **手动签到次数限制** — `/api/checkin-runs` POST 端点现在执行 `maxAttemptsPerDay` 校验
- **余额查询 userId 传递** — `new_api::fetch_balance` 现在正确发送 userId 相关 headers（之前被忽略）
- **更新接口返回值** — PUT `/api/accounts/:id` 现在返回完整账户 JSON（之前只返回 `{success: true}`）
- **用户名重复检查** — 创建用户前检查用户名是否已存在，返回清晰验证错误而非 500

### 性能优化

- **调度器并发控制** — 使用 `Semaphore(10)` 限制同时执行的签到任务数（之前无限制 spawn）
- **共享 HTTP 客户端** — OnceLock 全局复用 reqwest Client，30s 超时
- **数据库索引** — 新增 `CheckinRun(createdAt)` 索引，优化管理员全局记录查询
- **bcrypt cost 10** — 从 cost 12 降为 10，减少 1C1G 服务器登录耗时
- **SQL 简化** — `cleanup_checkin_runs` 使用子查询替代动态占位符拼接

### 时区修复

- **统一 UTC 存储** — 数据库所有时间戳使用 `DateTime<Utc>` 存储
- **本地窗口比较** — 调度器将 UTC 时间转换为本地时间后比较签到窗口和"今日"
- **计数查询修正** — `count_runs_by_account_today` 将本地午夜转换为 UTC 后查询，确保跨午夜正确
- **迁移格式统一** — 默认设置的时间戳改用 RFC 3339 格式（`strftime('%Y-%m-%dT%H:%M:%SZ')`）

### 代码重构

- **account_to_json** — 提取公共 JSON 构建函数，消除 accounts.rs 中 3 处重复
- **classify_checkin_status** — 提取签到状态分类到 `providers/mod.rs`，new_api 和 anyrouter 共用
- **前端 API 封装** — 提取 `api.ts`（getToken/authHeaders/request/apiUrl），4 个组件共用
- **错误响应解析** — `request()` 现在解析 JSON 错误体显示 `error/message/details` 字段
- **currentUser 传递** — AdminUserPanel 通过 props 接收 currentUser，移除重复 `/auth/me` 请求
- **CSS 清理** — style.css 移除 100+ 行无用 glassmorphism 样式，只保留 CSS reset + 动画

### 数据库

- `CheckinRun` 新增 `createdAt` 单列索引
- `CheckinAccount(ownerId)` 索引（已有）
- 迁移文件合并为单文件 `20260611_init.sql`（幂等 `IF NOT EXISTS`）

---

## v2.0.0 (2026-06-12)

### 新功能

- **用户管理** — 完整 CRUD，USER/ADMIN/SUPER_ADMIN 三级权限
- **所有权隔离** — `ownerId` 字段关联账户创建者
- **余额查询** — 支持 new-api 和 x666 站点实时查询
- **记录清理** — 管理员清理全部，普通用户清理自己的
- **自动调度** — Cron 签到窗口、重试策略、每日上限

### API 变更

- 新增 `POST /api/accounts/:id/refresh-balance`
- 新增 `POST /api/checkin-runs/cleanup`
- `GET /api/accounts` 支持 `?userId=` 筛选

---

## v1.0.0 (2026-06-10)

初始 Rust 版本发布
