# 当前 Spec 独立终态原文

Agent: `/root/preacceptance_spec_review`。只读，fixed `0258bcfd...3f4c0a6c`；后置 af81d911 修订不在此终态范围。

**Spec：PASS；hard findings：0；证据限制：1。** 范围为固定 `0258bcfd…3f4c0a6c` diff；已完整读取 request.txt。未运行测试、联网、读取真实凭据或修改文件。

已核实：

- 页面证据符合 request.txt:72–83 的“实际嵌入管理页面”“未知写入不盲目重放”。`scripts/acceptance/cpar-web-console-actual-ui.mjs:43–101` 经真实入口执行两渠道成功、身份 unavailable、丢失真实提交响应后的原任务回读、未知 apply 的 GET 核对及关闭重载；同时检查 SQLite、普通绑定和接收数。`docs/reports/assets/cpar-preacceptance-20261010/page/web-console-final.json:721`、`:1804`、`:2446` 分别保留两渠道未知结果及零推理证据，支持所声明的13项本地结果。
- 正式执行器满足 request.txt:123–132 的场景和计数准备：`cpar_new_build_acceptance.py:38` 定义3／2／4，`:859` 保留旧版本2范围，`:1234–1275` 执行9次并核对每轮实际身份、Usage及账本。`:475–520` 对初始完整库存、单候选、单账号、max_attempts=1、actual声明及 live批准／费用证明实行阻断。当前脚本／测试7个绑定hash及正式log hash均与归档相符。
- 首批清单未把模型选择当执行授权。`docs/reports/cpar-first-grok-authorization-20261010.md:3–18` 保留账号／环境／actual未知；`:35–46` 分开54基础、6条件Pin、目录及零重跑；`:54–60` 明确 visible-only cap不能形成费用上界。官方原文 `output/cpar-preacceptance-20261010/public-sources/rest-api-reference-inference-responses.md:29` 支持该限制，7份来源hash匹配。
- Kiro／Web取证任务逐项覆盖 request.txt:146–170：`docs/reports/cpar-native-evidence-tasks-20261010.md:17–23`、`:29–40` 给出材料、观测、权限、脱敏、停止条件及未来公共验收范围，未声称原生阻断解除。
- 三份原页面FAIL保持FAIL；产品 `apps/`、`crates/`、`web/` 在 c1b→固定提交间无diff。`cpar-preacceptance-local-20261010.md:75–89` 正确区分旧Full复用、新模拟oracle、真实NOT_RUN及全规格PARTIAL；GitHub后置同步尚不属于本次已核实成果。

限制而非当前准备任务失败：库存／actual声明门槛在命令开始检查一次（`cpar_new_build_acceptance.py:1237–1238`），后续HTTP发送`:1246`采用冻结快照，逐轮核验发生于响应后`:1267–1269`。因此证据证明初始隔离及事后停止，不能扩称运行期间配置漂移也会在每次付费发送前被阻止。真实执行仍须保持专用实例的冻结条件。
