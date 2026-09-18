# 问题跟踪器：本地 Markdown

本仓库的问题和规格说明以 Markdown 文件的形式保存在 `.scratch/` 中。

## 约定

- 每个功能使用一个目录：`.scratch/<feature-slug>/`
- 规格说明文件为：`.scratch/<feature-slug>/spec.md`
- 实现问题按票据拆分，每张票据一个文件：`.scratch/<feature-slug>/issues/<NN>-<slug>.md`，从 `01` 开始编号，不使用单个合并票据文件
- 每个问题文件顶部附近使用 `Status:` 行记录分诊状态（角色字符串可参考 `triage-labels.md`）
- 评论和对话历史追加在文件底部的 `## Comments` 标题下

## 技能要求“发布到问题跟踪器”时

在 `.scratch/<feature-slug>/` 下创建新文件；如果目录不存在则一并创建。

## 技能要求“获取相关票据”时

读取所引用路径中的文件。通常用户会直接提供文件路径或问题编号。

## Wayfinder 操作约定

供 `/wayfinder` 使用。**地图**文件中，每张票据对应一个**子文件**。

- **地图**：`.scratch/<effort>/map.md`，正文包含 Notes、Decisions-so-far 和 Fog 内容。
- **子票据**：`.scratch/<effort>/issues/NN-<slug>.md`，从 `01` 开始编号，正文包含待回答的问题。使用 `Type:` 行记录票据类型（`research` / `prototype` / `grilling` / `task`），使用 `Status:` 行记录 `claimed` / `resolved` 状态。
- **阻塞关系**：在文件顶部附近使用 `Blocked by: NN, NN` 行记录依赖。列出的所有文件都为 `resolved` 后，该票据才算解除阻塞。
- **前沿票据**：扫描 `.scratch/<effort>/issues/`，查找处于开放、未阻塞且未认领状态的票据；优先选择编号最小的票据。
- **认领**：开始工作前，将 `Status` 设置为 `claimed` 并保存。
- **解决**：在 `## Answer` 标题下追加答案，将 `Status` 设置为 `resolved`，然后在 `map.md` 的 Decisions-so-far 部分追加上下文指针（摘要和链接）。
