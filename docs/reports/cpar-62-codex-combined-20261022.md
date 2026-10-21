# CPAR #62 原样 Codex 组合受控正向验收

日期：2026-10-22，Asia/Shanghai。

**本轮执行目标 PASS：固定 Codex CLI 0.161.0 的原生 app-server stdio 模式完成正常工具闭环、真实磁盘续接、失败与同进程取消，Pi 回归保持。**
严格实际 runner exit=0、Codex 六项成功门槛全 true；最终 Full 45/45，Rust 1641/0/17。
这是指定配置与外层 OS 隔离下的 LOCAL_CONTROLLED 结果。整票 #62 仍 OPEN / PARTIAL / BLOCKED；
#61、固定 Build 1.0.50 服务资格、真实 Provider、远端 CI 与发布均独立。

最终不可变代码候选：`e4d5d4c1fedd2d3fa394a5a155eeb528d249c173`。
三项授权补充提交：`8de2d1a18dca8d598160410f84f4930a1277ea2f`。
报告与精选证据随后单独交付，不把代码验证改绑到文档提交。

## 实际基线、授权与最小产品变化

继续指定 worktree `/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020`，
分支 `feature/62-client-metadata`。本次授权续接时实际 HEAD 为 `684535c3`，没有回退到历史 SHA，
没有从旧 repo 开工。[前一阶段 BLOCKED 报告](cpar-62-codex-combined-20261021.md) 保留为历史证据。
用户于2026-10-22明确答复：“授权上述三项有界补充，继续完整验收”。

[ADR-0120](../adr/ADR-0120-bounded-codex-history-and-external-diagnostics.md) 与
[BC-PROTOCOL-014](../contracts/BC-PROTOCOL-014-bounded-codex-history.md) 冻结具体补充：

1. 无 id reasoning 仅接纳现有 summary 数组，content/encrypted_content/status 只能缺失或 null。
   不补 id、不造密文、不把摘要当 body，存在 id 的旧规则和 owned-token 授权保持。
2. function_call_output 自身 id 原值保留，沿用1–512 ASCII graphic 字节规则，加入同一有效历史的
   跨种类 item ID 去重；call_id 继续独立精确关联。增加独立 local-controlled history 资格。
3. [C4 合同](../contracts/BC-PROTOCOL-010-local-codex-client-metadata.md) 只新增 external /
   external-sandbox 配对诊断值；不完整／混合配对拒绝。完整原字段仍仅本地加密留存并授权读回，
   不发送 Build，不授予沙箱、审批或路由权限。

资格沿既有私有 cfg(test) 装配、owned loopback transport、依赖 controlled-test-support 接缝传递。
公开请求／URL／header／User-Agent／普通配置及其他字段资格均不能授予 history 资格。
正常网关候选与 Build 默认 builder/adapter 继续未资格拒绝；其他候选不继承新资格。
stock capability ledger 不变，受控 compiler 仍只单独增加本地 ParallelTools。
未新增 SQL、Canonical 字段、payload v1、reader、密钥/AAD/TTL、依赖或身份服务。

## 最终分场景结果

| 范围 | 结果 | 最终不可变运行的观察 |
| --- | --- | --- |
| 原样 Codex 正常 | PASS_LOCAL_CONTROLLED | 两个原生 PID 11720/11831 正常退出；三个 completed 回合、五增量200、五独立接收／Attempt／Usage／ledger；真实42换行文件 |
| 原样 Codex 失败 | PASS_LOCAL_CONTROLLED | 一个真实上游 Attempt 后 failed，客户端唯一 failed turn、唯一 error 且 willRetry=false；无 completed、工具执行或重发 |
| 原样 Codex 取消 | PASS_LOCAL_CONTROLLED | 真实 delta、lease=1、turn/interrupt；同 PID 11953 存活至清理前 peer close=1、lease=0；上界9ms |
| 未资格 Codex | PASS_REJECTION | 独立旧装配，HTTP400/ClientRequestError，receiver/Attempt/Usage/ledger=0 |
| Pi 正常 | PASS_BOUNDED_CONTROLLED | 三个 stop 回合、五增量200、两个相关 read/bash 工具、真实42换行，五条逐请求 join |
| Pi 失败 | PASS_BOUNDED_CONTROLLED | 一个 receiver/Attempt 后 error，durable Usage/ledger 精确20/7；既有错误界面零用量显示限制保留 |
| Pi 取消 | PASS_BOUNDED_CONTROLLED | 真实 delta/lease=1/RPC abort；同 PID 11585 活至 peer close=1/lease=0，上界15ms，清理随后发生 |

完整分场景回执见 [actual-cli-summary](assets/cpar62-codex-combined-20261022/actual-cli-summary.json)。
两个取消场景均分别记录一秒 fast probe 与20秒 bounded eventual，最终均 true。
上界是本轮采样观察，不是新增延迟 SLA。stop_process、EOF 进程收尾及 server.stop 只用于观测后的清理。

正常每次及失败场景的受控上游 Usage 为 input=20/output=7，provenance=Measured、input accounting=Inclusive；
均逐事件核对。未收到的 reasoning/cache 计数保持 None/UNKNOWN。取消 Usage/ledger 行数为0，
不推断实际 Provider 零用量或免费。native success companion 仍 None/UNKNOWN，不能用 C4 或 #59 代替 #61。

Pi 正常另有一个 background400 负探针，所以 durable Request/Finished 各6，实际正常请求、
receiver/Attempt/Usage/ledger 仍各5。401 在鉴权先拒绝，不产生上游增量。
Codex 的401、原未资格400及 Pi background400 均单独标 probe/负例，未删去真实客户端失败事件。

## 真实工具、历史与六项成功门槛

保持原生安装的 Mach-O binary，入口为 `codex app-server --listen stdio://`，没有 SDK 或手工
Responses 请求替代。正常顺序为第一用户回合三次 HTTP（exec→结果→write_stdin→结果→回答），
第二回合一次问答，正常关闭首进程后由新 PID 按真实 thread id 从磁盘 thread/resume，第三回合一次。
这次续接证明客户端磁盘历史；不外推未在实际客户端使用的网关 previous_response_id 或 compaction 模式。

实际 exec_command 在隔离目录读取 seed.txt=41，创建真实 PTY 并等输入。前一次真实工具结果返回
session ID **57266**，随后 write_stdin 实参必须为该整数，并输入 `1\n`。真实 shell 完成计算、
写入及读回 result.txt。测试进程只准备 seed，没有代写结果或生成工具返回值。

实际 call_id 为 call-62-0 / call-62-1，完整名称、namespace 缺失、参数对象、call/result 顺序、
自身 result id 和全文都精确校验。运行中工具输出为 `seed=41\r\n`，后续输出为 `1\r\n42\r\n`、
exit code=0；文件原字节为 `42\n`。完整动态结果与参数在汇总的 ordered_tools 中留存。

| 原严格门槛 | 最终结果 |
| --- | --- |
| 客户端正常结束 | true，两个实际原生进程 exits=[0,0] |
| 三个完成回合 | true，三个匹配 thread/turn 的 completed 通知，无 error/failed |
| 五次增量公开200 | true，五个完整且唯一的 response.completed、正确 status/model/Usage，无 failed/error 或截断捕获 |
| 五次独立接收与精确 accounting | true，五条唯一 Request/Attempt/receiver/response/Usage source/ledger 关系 |
| 两个准确关联工具结果 | true，完整参数、call_id、自身id、namespace presence、全文及顺序 |
| 真实文件42 | true，精确42加LF |

Codex 会在完整 response.completed SSE 记录后停止读取，不继续轮询到 HTTP EOF。
因此 HTTP EOF 观察与语义完成分开留存；本轮不把未观察 EOF 改写为 true。
严格解析要求原始 wire 已有完整空行边界，不拼补分隔符；少末尾一／两个换行均失败。
既有五次200、唯一终态、无错误和真实 completed 回合的成功含义与计数保持。

## 实际组合字段与逐请求证据

每个公开 x-request-id 连接 durable Request 和唯一实际 Attempt；实际 response.created.id 连接
独立 receiver ordinal/response id 与 Usage.response_id，ledger 再连接真实 Usage source_event_id。
C4 从原始 HTTP 请求字节的 RawExtensions 提取，验证同 owner/Request/实际 Attempt 的完整加密授权读回，
完整 raw JSON 一致，foreign owner／错误 Attempt 拒绝。此次七个已发送 Codex 请求均有 C4，
正常五次、失败一次、取消一次分别验证。合法后续缺席分支仅经判据回归，未声称本次客户端省略过它。

C4 reader 返回 canonical durable Attempt，event reader 还挂载单独、serde-skip 的 execution identity。
修正后的 oracle 比较全部序列化 Attempt 字段，另要求实际 execution identity 存在，未删除关联断言。

| Codex HTTP轮次 | 客户端input项 | 非assistant message ID数 | 无id reasoning数 | 工具结果数 |
| --- | --- | --- | --- | --- |
| 0 | 3 | 3 | 0 | 0 |
| 1 | 6 | 3 | 1 | 1 |
| 2 | 9 | 3 | 2 | 2 |
| 3 | 12 | 4 | 3 | 2 |
| 4（新PID恢复后） | 15 | 5 | 4 | 2 |

五轮实际均有 multi_agent_v1 namespace、精确 web_search(false)、parallel=true、summary=auto 无 effort、
include=[reasoning.encrypted_content]、C4 external/external-sandbox。完整输入顺序、各 id presence/value、
reasoning null/absent、工具和结果都与独立上游逐轮比较；仅沿用既有 instructions→developer 与 message
type/string-content 投影，没有删改实际请求字段。[字段回执](assets/cpar62-codex-combined-20261022/actual-field-observations.json)
按实际公开 Request ID 留存各轮变化。

C1–C3 的原生声明／身份与相关控制、C4、C5、C6 和 parallel 在实际路径共存。**namespace 子工具与
hosted search 均 NOT_CALLED**；实际两个工具跨响应依赖执行，parallel=true 是控制共存证据，
不是本场景同响应多调用执行证据。既有 JSON/SSE 合成组合、ParallelTools 多调用和 stored-history
专项保留在 Full，未外推成实际未调用能力通过。

## 原生重试配置与保留的失败

临时 provider 配置明确使用固定版本支持的 request_max_retries=0、stream_max_retries=0。
未改变工具声明、并行/search能力、请求字段、账号或模型。只声明此配置结果，不声明客户端默认
重试策略通过。固定版本 [model-provider-info.rs](https://github.com/openai/codex/blob/979011409de0a60b52f179721948e65531d26144/codex-rs/model-provider-info/src/lib.rs#L492)
与完整源文件 SHA 冻结在 [source manifest](assets/cpar62-codex-combined-20261022/source-manifest.json)。

开发3保留了默认stream retry导致第二接收、旧 peer 后续成功兜底、false completed 的真实 FAIL。
failure peer 现在对任何意外重发都继续返回失败，接收数和 Attempt 仍必须精确为1。
开发1的 C4 Attempt 比较失败、首次参数接线遗漏编译失败、reqwest未开启json方法的装配编译失败、
各轮 Clippy（crate docs、Debug field、函数长度）失败均保留，没有放宽 lint 或增加超时。

独立 Spec 初审的两项 P2 也保留 RED/GREEN：C4合法后续缺席误拒、SSE缺空行误受。
C4 mutation 是复制真实回执后的纯 oracle 测试，明确不是实际客户端验收证据；原请求未改。
SSE 新回归先实际失败再修正，Python 三项回归覆盖缺席证明、完整读回和共同缺失不能通过。
[反例绑定](assets/cpar62-codex-combined-20261022/oracle-regressions.json) 与
[独立审查](assets/cpar62-codex-combined-20261022/reviews-final.json) 保留初审与关闭结论。
开发中间运行不改绑为最终候选；未完整冻结每次中间 binary 的，仅作开发证据。

## 最终验证、身份与隔离

最终候选使用自身 manifest/lock，在 source worktree 加明确18项继承覆盖层运行；这是 overlay Full，
没有称 bare-tree Full。7,247项源文件／manifest／覆盖输入在 Full 和实际 CLI 前后逐项验证字节、mode、
uid/gid、symlink一致。[候选绑定](assets/cpar62-codex-combined-20261022/candidate-binding.json) 与
[稳定性](assets/cpar62-codex-combined-20261022/input-stability.json) 不包含后续报告提交。

- Full：45/45、exit0、826.317秒；Rust1641 passed /0 failed /17 ignored，131 suites。
- 严格实际 runner：exit0，54.507秒，七场景完成，六项成功门全true，无 missing receipts；实际 ignored 子测试另行显式运行。
- 开发受影响检查：core/router/Responses285/0/1，provider lib33/0/0，公开Responses80/0/2；最终 Full 包含新增SSE回归。
- 受影响 Clippy 全目标／全特性、-D warnings 通过；最终候选 Python oracle3/3通过。
- Standards：完整23源路径初审及最终五文件增量复核均PASS，0 hard/0 judgement。
  Spec：初审两P2已复现修复并独立关闭，本轮执行范围PASS；整票仍PARTIAL/BLOCKED。

Codex launcher/native均0.161.0，原版Pi shell/声明bin均0.84.3，Node v26.8.2；only-my-pi仅辨识未执行。
12,564项安装文件／链接前后完全一致。每场景实证临时HOME/CODEX_HOME/工作区、合成凭据、
IPv4/IPv6非loopback拒绝、真实用户目录读取拒绝、临时区外普通写入拒绝，均EPERM。
保留外层 Seatbelt，PTY例外仅/dev/ptmx与数字/dev/ttys设备。externalSandbox声明不替代这些探针。

| 绑定对象 | SHA-256 |
| --- | --- |
| 实际Codex native | `12ac11d2c7eee27cfae34393986d7b7c9ed0dea537cb749831cdd7033893e6de` |
| 最终Python runner | `cd4ac44b777c8037c236c1f4c20131124ad68f7a1cf0e6a03b5cc2169d0b3e18` |
| Cargo.lock | `36564c0219707741d79ecb37d9fc4ee620ce2fe6801fdbe09b32fb4c8687c858` |
| Full日志 | `ed5176da23aa5edbd9a3b4a10d82c07674bc5198f8710e61a644b3a2fe45a397` |
| Full实际gateway test binary | `cc61b1eefc8c8779c155039a1c0c2611a5afaf6a3b8d60de979d0dd82ac42418` |
| 实际CLI test binary | `db4694d3e2223bce43134b21fd4d57cc138443dd672029b4d159f225684cf4cc` |

Full 与 package-only CLI 的 feature unification/binary分开记录，两个实际binary均独立留存。
14份完整来源内容重新核对SHA；未知 publication date 保持null，抓取日期没有充作发布日期。
起始18项内容保留；两份共改文档只提交本轮追加段落，未纳入继承hunk。
详见 [preservation](assets/cpar62-codex-combined-20261022/preservation.json)。

## 复现与剩余条件

在指定worktree运行Full；严格runner输出必须是OS临时目录下尚不存在的新路径。
以下命令保留隔离、版本检查、真实工具、零重试配置和两项严格门禁：

```sh
cd /Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020
CARGO_TARGET_DIR=/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-codex-combined-20261022/target-full-final ./scripts/check.sh full
python3 scripts/acceptance/test_cpar62_cli.py -v
python3 - <<'PY'
from pathlib import Path
import os, subprocess, tempfile, uuid
cpar62_repo = Path('/Volumes/Asgard/Agentprojects/CPA-Rust/worktrees/cpar62-client-metadata-20261020')
cpar62_env = os.environ.copy()
cpar62_env['CARGO_TARGET_DIR'] = '/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-codex-combined-20261022/target-full-final'
cpar62_output = Path(tempfile.gettempdir()) / ('cpar62-reproduce-' + uuid.uuid4().hex)
subprocess.run(['python3', str(cpar62_repo / 'scripts/acceptance/cpar62-cli.py'),
    '--require-prompt-release', '--require-codex-success', '--output', str(cpar62_output)],
    cwd=cpar62_repo, env=cpar62_env, check=True)
print(cpar62_output)
PY
```

本地私有证据根：`/Volumes/Asgard/Agentprojects/CPA-Rust/output/cpar62-codex-combined-20261022`。
按允许列表独立复制校验489份原始回执，raw目录0700、文件0600，原临时运行目录也保留。
上一批416份原始回执与所有历史失败保持原状，见 [历史引用](assets/cpar62-codex-combined-20261022/historical-evidence-reference.json)。
完整源码输入清单、原始Full/CLI/失败日志、安装清单、固定来源和实际binary在私有根；
Git只纳入报告及精选摘要/hash，不纳入原始C4、临时凭据配置或数据库。

[GitHub只读快照](assets/cpar62-codex-combined-20261022/github-status.json) 确认 #62/#61仍OPEN，
#62原生blocked_by=#61；#61的#59/#60已closed。main/develop与功能分支远端状态未变。
**整票#62仍OPEN / PARTIAL / BLOCKED；固定Build 1.0.50为BUILD_ACTIVATION_BLOCKED。**
#61成功原生companion及cost_in_usd_ticks仍独立，费用不填零；Pi失败界面零显示仍为已知限制。
真实Provider/付费账号、远端CI、发布均NOT_RUN。本轮没有push、PR、工单/依赖写入、合并、发布或部署。
