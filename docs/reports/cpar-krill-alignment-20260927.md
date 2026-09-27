# Krill 模型配置核对（2026-09-27）

状态：用户明确选择停用旧路由、接入gpt-6-sol并创建单模型测试Key后，配置已发布并回读验证。下文核对记录保留修正前证据。

用户授权先处理Krill模型配置不匹配。本轮通过现有管理鉴权对Krill两个精确Endpoint/Credential各执行一次metadata-only目录刷新，再完整分页读取目录、路由候选及有效Key模型投影。未执行推理、未替换凭据、未修改已开放模型或Key权限。活动配置前后相同。

## 已确认事实

- Chat Completions、Responses两个接口实时各返回21个模型，集合一致，均不含`gpt-5.5`。这证明当前目录不支持以它作为验收依据，不证明目录之外的调用一定会失败。
- Krill唯一已配置候选是Chat Completions上的`gpt-5.5`，上游ID也为`gpt-5.5`。
- `gpt-5.6-terra`虽同时出现在Krill目录和公开模型列表，现有候选实际指向Codex，并非Krill；不得把同名视为Krill已经接入，也不得静默改变其来源。
- 两个有效Key的有效模型列表均为`gpt-5.6-terra`、`grok-4.5`、`grok-4.20-0309`，不含`gpt-5.5`。这是有效权限投影，不等同完整原始授权图不存在该路由授权。
- 没有为发现的21个模型自动创建路由或扩大Key权限；目录中包含图像模型，但本轮不会据目录可见就开放未支持媒体协议。

## 待决修正

已向用户提供三个具体选项：先停用遗留`gpt-5.5`路由且保留Key权限；或停用后接入`gpt-6-sol`并创建单模型测试Key；或由用户指定exact模型及目标Key。任一方案都不能把旧模型名映射到未经确认的新模型。

用户选择后再准备具体版本差异、校验/发布及回读，保留其他渠道与账号/历史。需要推理验收时先登记目标，每次最多512输出token、无自动重试，累计额度仍10/12。

## 证据

本轮metadata刷新2次，推理0次。脱敏证据见[目录与有效权限](evidence/cpar-reliability-m4-20260926/krill-inspection.json)、[候选来源](evidence/cpar-reliability-m4-20260926/krill-routes.json)。本地脚本在`output/krill-alignment-20260927/`；管理认证仅在远端内存使用，证据不含凭据或账号身份。

## 已授权配置修正结果

- 从当前活动配置fork隔离草稿，关闭`gpt-5.5`公开模型及其Krill候选，保留旧记录而非删除历史。
- 新建原名`gpt-6-sol`，仅连接Krill Responses接口，canonical_bridge、max_attempts=1，不使用allow_unlisted例外或新增别名。
- 新建独立访问组与7天有效测试Key，仅授予新路由。已有Key记录不变，发布前后有效模型及来源逐项一致。
- 配置校验通过，使用revision、预期活动版本和lifecycle event防并发发布，再应用运行时；没有更换gateway二进制或部署未发布的前端文案。
- 新Key管理有效投影及数据面`GET /v1/models`均只返回`gpt-6-sol`。没有发送推理请求；这证明模型权限接通，不证明上游推理成功。累计真实额度仍10/12。
- Key已保存到本机`output/krill-alignment-20260927/test-key.json`，权限0600，未提交仓库或输出到聊天；远端生成时的副本亦为0600且父目录0700。
- 自动化首次误用GET调用校验接口而收到404，发生在草稿阶段、未发布；随后核对OpenAPI，恢复同一草稿并使用POST完成校验，没有重复创建Key或重放写入。

发布与权限证据：[配置回执](evidence/cpar-reliability-m4-20260926/krill-publication.json)。回退基线为回执中的previous_version；后续回退须重新核对并发配置及当前凭据，不自动恢复旧快照。当前只涉及这次模型与新测试Key配置，账号、历史、其他渠道保持。
