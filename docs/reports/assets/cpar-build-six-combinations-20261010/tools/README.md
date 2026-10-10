# 本批执行工具的归档边界

这些工具记录六组合批次的实际准备、预算与停止机制，调用的是正式 `cpar-batch-c-live.py --new-build` 及其原执行器／collector。它们不替代基础场景。

本批预算已经 `stopped=true`，真实执行结束。不得解除停止标记、删除 summary 或用这些脚本重跑。后继需要新的授权、epoch、身份冻结与预算。

- `remote-prepare-v1.py` 是实际首准备原字节，SHA256=`c19dd7f53df361a06f461a5756308eeb05a90a175097750bd2163fa9b38565b2`；它只复制了部分 key，首次启动因缺 management-csrf 在0发送时失败。
- `remote-prepare.py` 是该准备错误的后继最小修正参考，增加从原自有材料复制必要 key 的列表；本批没有重新执行整个 prepare。
- `remote-start-v1.py` 是实际失败启动原字节，SHA256=`0ed5c0f81e4fbe6770bc4583a89e9e6597ffeff74ed6e9d13c5f756817e07afe`。
- `remote-start.py` 是实际第二次启动，使用新日志／launch 文件保留 v1，SHA256=`ec778b23caf89c5ebe173046994bb43c61e7417c7f0095d587babbb3152302d7`。补齐 key 的一次性操作及0发送诊断留在保护区 startup-repair.json 和旧日志中。
- `remote-pin.py` 实际执行 Responses JSON Pin一次；`remote-basics.py` 实际执行5次基础后停止。两者绑定真实执行的 formal SHA256=`2107227d545acb898ea6b036bdcef6ec5fc030e266b880d63f876b62746a9f48`，没有绑定后来修复的 formal 文件。
- `remote-posthoc.py` 只读补采已经返回的第五响应及诊断，不发新推理；`remote-exit.py` 停止自有 unit、核对生产不变量／请求增量／旧材料／私有权限。

远端原始请求、响应、Pin、账号材料、数据库、运行观察及失败日志保存在批准保护根的 `private/six-combinations-20261010/`。公开资产只给脱敏结果、opaque身份、hash及权限；不提交凭据、数据库或 wire 内容。上轮 `private/revalidation-20261010/` 和 `private/phase2/` 保留。
