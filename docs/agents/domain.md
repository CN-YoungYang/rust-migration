# 领域文档

工程技能探索本仓库代码时，应按照以下规则使用领域文档。

## 开始探索前先读取

- 根目录的 **`CONTEXT.md`**；或者
- 如果根目录存在 **`CONTEXT-MAP.md`**，读取它指向的各个上下文 `CONTEXT.md`，仅阅读与当前主题相关的内容；
- **`docs/adr/`**：读取涉及当前工作区域的架构决策记录（ADR）。对于多上下文仓库，还要检查 `src/<context>/docs/adr/` 中与当前上下文相关的决策。

如果这些文件或目录不存在，**静默继续**。不要专门提示它们缺失，也不要在开始工作前建议提前创建。通过 `/grill-with-docs` 或 `/improve-codebase-architecture` 触达的 `/domain-modeling` 技能，会在术语或决策真正明确时按需创建它们。

## 文件结构

单上下文仓库（大多数仓库）：

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

多上下文仓库（根目录存在 `CONTEXT-MAP.md`）：

```
/
├── CONTEXT-MAP.md              → 系统级上下文地图
├── docs/adr/                   → 系统级架构决策
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/           → 上下文级架构决策
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 使用术语表中的词汇

当输出中出现领域概念（例如问题标题、重构提案、假设或测试名称）时，应使用 `CONTEXT.md` 中定义的术语。不要改用术语表明确避免的同义词。

如果所需概念尚未出现在术语表中，这说明要么正在使用项目未采用的语言（请重新考虑），要么领域文档确实存在缺口（请记录给 `/domain-modeling`）。

## 标记 ADR 冲突

如果输出内容与现有 ADR 冲突，应明确指出，而不是静默覆盖：

> _与 ADR-0007（事件溯源订单）冲突，但由于……，值得重新讨论。_
