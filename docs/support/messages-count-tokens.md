# Messages 输入计数支持范围

2026-10-25；#68；决定为 [ADR-0127](../adr/ADR-0127-cpa-compatible-token-counting.md)，
行为契约为 [BC-PROTOCOL-003](../contracts/BC-PROTOCOL-003-exact-token-count-capability.md)。
本页描述生产代码的装配与限制。本地受控验证不代表实际客户端、真实 Provider 或发布资格。

`POST /v1/messages/count_tokens` 使用现有客户端认证、模型开放范围、Alias/原始模型解析和
运行代快照。成功体仍只有 `{"input_tokens": N}`。无需客户端插件或新的请求字段。
成功响应的 `x-cpar-token-count-method` 是固定的非秘密方法标识：

| 标识 | 来源与含义 |
|---|---|
| `provider-reported` | 所选上游的计数 API 报告；不承诺后续推理实际消耗相等 |
| `proven-exact-local` | 显式证明适配模型的本地精确能力；类型接口保留，生产未安装这类 tokenizer |
| `estimated-cpa-o200k-base` | 固定 CPA 算法的本地估算，不是模型精确 tokenizer，也不是实际 Usage |

已有 exact Provider capability 仍只接受 `ExactInputTokenCount`，其 Router 适配器标记为
`provider-reported`。估算分支只构造 `InputTokenCount::Estimated`，不存在估算转精确的转换。

## 渠道和模型

- 对 `anthropic-compatible.messages` 且源为 HTTPS `api.anthropic.com:443`、完整 Messages
  路径为 `/v1/messages` 的候选，调用 `/v1/messages/count_tokens?beta=true`。使用已选凭据的
  API key 或 OAuth 授权，固定 Anthropic version 和 counting beta。标准路径可以由 base/path
  两段组成。第一方非标准路径明确拒绝，不改成估算。
- 自定义 Anthropic origin 以及现有其他已注册渠道，采用本地估算；包括映射到 Chat/Responses、
  Grok、Kiro 的 Messages 输入。只要模型通过现有开放范围与运行态候选/凭据检查就可估算，
  不以名称猜测 tokenizer 精度，也不因此宣称该渠道所有推理能力已获得资格。
- 原始 upstream model 与其 Alias 保留 exact-model 筛选。普通映射继续使用现有 Provider-scoped
  排序；多个 Provider 不明确的映射仍拒绝。陈旧快照、不可用凭据、运行态健康/额度阻断均不能
  通过计数绕过。没有计数重试、自动账号切换或上游失败后的本地 fallback。

## 输入与算法

生产计数支持最多 **256 KiB UTF-8 JSON**；HTTP 原有整体 body 上限继续有效。
支持文本/system 文本，普通工具定义（name、description、对象 input_schema）、工具调用
（id、name、对象 input）及文字工具结果历史，普通 thinking 文本与 `tool_choice`。
文本 cache control、thinking signature、工具结果 is_error 可保留，但不添加估算片段。

图像、音频、视频、文档、redacted thinking、服务器/MCP/搜索工具、citations、未知内容块、
未知输入字段明确拒绝，不静默丢弃；该限制同时适用于当前 native 和估算路径。
工具结果仅支持字符串或 text 块，未知嵌套内容同样拒绝。

固定参考为 [CPA v8.0.40 / d946c4f](https://github.com/router-for-me/CLIProxyAPI/blob/d946c4fba6fc9a5cfbca1f3d85184d62ad2fb683/internal/runtime/executor/helps/claude_input_tokens.go)：
先 system，再每条 role/content，再工具定义，最后 tool_choice；片段 trim 后去空、换行连接，
一次使用 `o200k_base` 普通文本 BPE 计数。工具 input/schema 紧凑化保留键顺序、数字原文与
字符串转义。特殊 token 字符串仍按普通文本计数。Rust tokenizer 固定 `tiktoken-rs 0.12.0`。
CPA 对若干多模态内容的忽略行为在本票被明确拒绝取代，以满足 #68 的不丢内容要求。

`max_tokens`、采样/停止控制、thinking 配置、output_config、metadata、context_management、
diagnostics 不增加估算片段。Native 请求按固定计数边界去掉非计数的生成控制以及
metadata/context_management/diagnostics，保留输入内容和 thinking。模型写为实际已选 upstream model。
本地估算不模拟不同模型的隐藏提示、缓存、工具包装或实际生成转换，因此**没有通用误差上限承诺**。

## 故障、资源与账本

认证、模型不可见、重复 JSON 字段、畸形/超限输入、未知内容都有安全错误；错误不包含
输入文本、工具内容、Endpoint 或凭据。计数 API 的非 200、非 JSON、缺失/重复/非整数/
负数/溢出计数和超过 64 KiB 的响应全部失败。上游返回的合法零值可以交付，缺失值不能补零。
上游传输沿用 DNS/egress admission 与已选凭据的出口代理，30 秒 deadline、禁止自动重定向。
本地 BPE 在线程池运行，并持有原有并发租约至计算完成；取消等待不会提前释放仍在计算的容量。

计数不调用推理接口，不进入 inference orchestrator，不创建 Request/Attempt/Usage 推理事件，
不写计费 ledger，也不修改价格、额度历史或现有推理账本口径。
方法标识只说明计数来源，不含提示词、工具内容、凭据或 endpoint 信息。

## 可重复样例

[固定样例](../../tests/fixtures/count-tokens/cpa-v8.json)由上述 Go commit 的原始收集函数及
`github.com/tiktoken-go/tokenizer v0.8.1` 在独立本地程序实际计算。Rust 回归比较同一请求原文：

| 样例 | Go 参考 input_tokens |
|---|---:|
| text | 4 |
| system | 7 |
| unicode | 16 |
| special-strings | 16 |
| tools | 19 |
| tool-history | 55 |
| thinking | 11 |
| whitespace | 5 |

这些是算法兼容样例，并非与真实模型 Usage 的准确率评估。真实官方计数、真实 Provider、
实际 Claude Code/Codex/Pi 和固定 Build 服务没有在本票调用；相关资格仍由 #64 等渠道票承接。
