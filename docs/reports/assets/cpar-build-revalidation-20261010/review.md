# 本批两轴审查与范围

本记录保留本批失败审查及后继修复，不将修复后的局部 PASS 写成 #47 完整验收通过。主代理负责代码修改、方案选择、实际验证与最终汇总；子代理仅只读核验。

| 精确范围 | Standards | Spec | 结果承接 |
| --- | --- | --- | --- |
| `910cad67..f9412895` | PASS，hard 0 / heuristic 0 | FAIL，P1 旧授权 binding 未绑定 Pin 后声明；revision 浮点边界需拒绝 | 原 FAIL 保留；后继 `a6aa31df` 修正 |
| `f9412895..a6aa31df` | PASS，hard 0 | FAIL，P1 再冻结可能替换既有 Client Key／projection 范围 | 原 FAIL 保留；后继 `3bf9acd3` 修正 |
| `a6aa31df..3bf9acd3` | 主代理静态核对局部标准；前一独立 Standards 保留其原范围 | 独立 PASS，P1 CLOSED | 重新冻结仅允许 observation 时间推进；其他旧字段逐项保持 |
| `3bf9acd3` 后 nullable／verdict 修复，字节绑定见 [review-nullable-scope.json](review-nullable-scope.json) | 独立 PASS，hard 0 / heuristic 0；覆盖修复、测试及当时三份 runner／退出脚本 | 独立小范围 PASS；仅两行修复、三个测试、Responses 公共契约和回执行段 | 最终 38 tests PASS；整个 #47 Spec 仍 PARTIAL |

最后一轮 Standards 已核验 `remote-basics.py`、`remote-exit.py`、`remote-finalize.py`、实际退出回执和原退出失败，不声称其亲自执行测试或远端复核。小范围 Spec 对 `scripts/acceptance/cpar_new_build_acceptance.py:119`、`:921` 和 `scripts/test-cpar-new-build-acceptance.py:180` 给出 PASS：Responses 接受缺失／null error，所有非 null error（含假值）仍拒绝；failed／incomplete 不能成功；其他协议规则不变；execution FAIL／BLOCKED 控制 basic 总结而未完成场景保持 NOT_RUN。

最终代码字节：formal `2107227d545acb898ea6b036bdcef6ec5fc030e266b880d63f876b62746a9f48`；tests `95e26bfcb2737acacc03715d0bde7d7c393b69c9cfb6e6001ecc33c2445a9a54`。审查基线 `3bf9acd3ad28093cba4d789f996e3ba1bcdab630`。真实执行用的是修复前 formal `a46a2880e9614048411a25f594b0532c582e0a2ba29ab32d090068d269b53683`，原字节留在远端保护目录。修复后的 formal 没有执行真实请求。

较宽的最终 Spec 探索及依赖探索在催收后未返回终态，已中断，记 PARTIAL，未计为 PASS。随后重新派发干净、限定两行修复的 Spec 核验取得终态 PASS；外围 GitHub 依赖由主代理实时 `gh` 读取。后增 `remote-posthoc.py`、`remote-seal.py` 和最终报告／资产由主代理逐文件读取、静态检查及实际回执复核，未冒称获得独立全范围 Spec PASS。

原真实 FAIL、原错误 basic `NOT_RUN`、原退出权限断言失败、离线 `runtime_observation_stale` BLOCKED 全部保留。单次公共响应的事实与完整多轮／工具／扩展验收分别记录，不以已收到的第一轮替代完整场景。
