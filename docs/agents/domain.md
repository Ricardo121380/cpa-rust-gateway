# Domain Docs

## Layout

本仓库采用 single-context：

- `CONTEXT.md`：根目录领域模型与术语表，按需创建。
- `docs/adr/`：仓库级架构决策，沿用现有内容。
- `docs/adr/README.md`：ADR 编号、状态、结构及维护规则。

Rust workspace 中的 crates 继续共用此入口。

## Before exploring, read these

1. 若根目录存在 `CONTEXT.md`，完整阅读。
2. 阅读 `docs/adr/README.md`，再完整阅读与当前任务相关的 ADR。
3. 若以后出现 `CONTEXT-MAP.md`，先阅读该映射，再完整阅读
   当前任务涉及的各个 CONTEXT.md 及其领域 ADR。

缺少 CONTEXT.md 或 CONTEXT-MAP.md 时直接继续。
领域术语或决策实际明确后，由 `/domain-modeling` 按需建立文档。

## Use the glossary's vocabulary

Issue 标题、方案、假设和测试名称中的领域概念，采用
CONTEXT.md 定义的词汇，尊重其中明确区分的概念。

缺少必要术语时，先确认是否已有对应概念；
确有缺口时，记录给 `/domain-modeling` 处理。

## Flag ADR conflicts

方案与现有 ADR 冲突时，明确引用 ADR，并说明重新讨论的原因。
决策变更遵循 `docs/adr/README.md`：
新建 ADR，并在旧记录中链接替代项。
