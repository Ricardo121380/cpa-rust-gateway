# Spec 修订独立失败终态原文

Agent `/root/final_acceptance_spec_delta`；固定 `3f4c0a6c...af81d911`。此原失败保持FAIL，修正后另独立复核。

Spec delta：**FAIL；hardfindings=1（P2）**。范围仅固定 `3f4c0a6c…af81d911` 的9文件。

- P2：已发送的历史bundle可能被回报为发送0。`scripts/acceptance/cpar_new_build_acceptance.py:857`新增 `sent_requests=0`；preflight失败在`:860–862`直接返回，跳过`:864`已有samples计数。事实：`audit`读取历史bundle（`:1326–1327`）；`verify_runtime`在观察超过600秒时拒绝（`:413`）。静态可达反例，未运行：已完成9次发送的bundle，11分钟后执行audit，得到 `BLOCKED/runtime_observation_stale`及 `sent_requests=0`。证据过期不能证明未发送；关联 `request.txt:130`动作计数及`:180–181`实际执行归属、原失败保留要求。
- 费用修订符合 `request.txt:132`：`:518`强制六类coverage含 `tool_output`，由`:1239`在HTTP发送`:1247`前验证。新增负例覆盖六protocol/mode（测试`:570–571`），移除tool_output后断言实际receiver未增加（`:647、652`）。
- 已有修订测试记录核对：`output/cpar-preacceptance-20261010/clean-formal-oracle.log:35–37`为32 tests／OK；本轮未运行测试。报告`:57`明确冻结观察限制；`:27`区分额度15／停止后18；`page-execution-index.json:17、92`如实保留启动时间缺失。上述文案未发现新增硬问题。
