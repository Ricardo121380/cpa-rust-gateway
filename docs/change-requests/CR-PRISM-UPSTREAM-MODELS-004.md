# CR-PRISM-UPSTREAM-MODELS-004 · 上游模型清单与原名接入

2026-09-13。用户已确认：一个上游可以有多个模型，同名模型可以有多个来源；默认保留真实模型 ID，别名可选，目录与客户端授权分开。前后端由当前 Codex 实施。

## 缺口

现有 catalog/status 只给数量，effective models 是已授权运行投影，无法作为未配置模型的完整上游清单。模型配置 UI 强迫输入客户端名，重复名称会要求另起名字，缺少同模型多来源接入。

## 契约

新增管理鉴权、配置范围下的 GET `/admin/catalog/models`。必填 endpoint_id、credential_id，limit 默认100、上限100，可选 cursor、q。只读取已有持久目录，不触发 Provider 请求、不接收浏览器 Client Key secret。
返回 config_version、revision、target（endpoint_id、credential_id、snapshot_version、observed_at_ms、stale_at_ms、expires_at_ms）、items（model、present_in_last_success）、next_cursor。缺少成功目录返回404，空目录返回200空数组，硬过期保留证据但不表示可接入/可调用。

分页在独立只读 SQLite 事务中执行，WHERE 精确范围、模型稳定键及 LIMIT 下推；游标绑定配置修订、目录版本/观测时间、target 和搜索词。任一来源改变返回409并要求重读，不拼接不同版本。只打开现有受管接口的目录；原生账号与普通凭据同样使用已有目录证据，不读取秘密。

## 前端与迁移

提供商按接口展示已接入模型和目录入口。一次选择/粘贴多个 exact ID；默认原名，同名增加候选来源而不是新增模型或强制别名。模型主页按模型展示来源/协议/状态，名称映射与底层路由放高级入口。

生产旧六个验收名称在私有备份/隔离验证后整理：三个唯一模型改原名；三份 gpt-5.5 在确认授权组集合相同、实际模型/账号来源不变后合并连接。旧名称保留兼容别名，旧请求/账本不改写，不增加原来未授权的模型。不删除账号、密钥、原始历史，也不扩大当前已支持 discovery 渠道。

## 验证

目标隔离、空/缺失/过期、分页及目录/配置变化409、大小边界；原名默认与可选别名、同名不同来源、重复连接不新增、不同模型不合并；真实 gateway 上一个上游至少两个模型、同模型至少两个来源，客户端实际请求 model 原样到 loopback mock。生产副本验证迁移后权限、兼容别名、运行加载与回滚，再上线检查。

## 实际数据面验收发现与后端定稿

多来源实际调用暴露了旧 Provider-scoped 执行入口对多个提供商的拒绝。
本轮新增 **授权 exact model** 的执行入口：只接纳该原始 ID 的显式候选，单 Provider
保留原有价格策略，多 Provider 使用配置的 Route 优先级/权重，不跨 Provider 偷换价格比较。
共享原有快照、重试预算、协议语义过滤、Health/Quota、目录硬过期及原子 Credential 租约。
自定义名称混合映射仍保留原有 Provider 范围约束；Channel Pin 和 stored continuation
仍固定原来源。原名模型的显式别名继承其 exact model 范围。

Route Explain 对原名多来源使用同一 Route 调度投影，先过滤协议；自定义混合模型仍要求
Provider 上下文。新增路由回归证明跨来源失败切换、错误 exact ID 零租约/零调用，既有
Provider-scoped 模糊拒绝和续接隔离测试继续执行。接口形状与 schema 26 均不改变。
