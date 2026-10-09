import copy,json,pathlib,re,subprocess
root=pathlib.Path.cwd()
out=root/'output/cpar-continuity-operations-20261009-171952'
assets=root/'docs/reports/assets/cpar-continuity-operations-20261009-171952'
assets.mkdir(parents=True,exist_ok=True)
matrix=json.loads((out/'initial-issue57-matrix.json').read_text())
issue9=json.loads((out/'issue9.json').read_text())
history=json.loads((root/'docs/reports/assets/cpar-batch-c-convergence-20261001/combination-ledger.json').read_text())
source='6a37f74d63258d283da5f55e5e5ef31b9b9ed0c4'
matrix.update(runtime_source=source,overall='PARTIAL',E05='AWAITING_USER_DECISION',current_device_loop='PASS_LOCAL_SYNTHETIC_STOCK_PRODUCTION_TRANSPORT',current_api_5_by_3='PASS_LOCAL_ACTUAL_PAGE',real_provider_calls=0,remote_ci='NOT_RUN',artifact_candidate='NOT_RUN',deployment='NOT_RUN',production='NOT_RUN')
api='final-api-5x3-green.json.gz'
device='final-device-main.json.gz; final-device-boundaries.json.gz'
pointer='final-desktop-pointer.json.gz'
historical='cpar-seo-closeout-local-20261009.md / source c3d9dd949c1fab69042da97deb9482a0aa14fd47 / original 2026-10-09 UTC retained'
for row in matrix['stories']:
 n=int(row['id'][2:]);row['status']='PARTIAL';row['current_source']=source
 row['evidence_role']='historical originals plus this batch limited acceptance; whole story not automatically accepted'
 row['current_slice']='NOT_RUN: outside focused page slice'
 if n<=23:
  row['implementation']='web/prism/src/features/upstreams/ProviderDialog.tsx; web/prism/src/features/upstreams/apiProviderModel.ts; web/prism/src/features/accounts/KiroDeviceDialog.tsx; web/prism/src/features/accounts/KimiDeviceDialog.tsx; crates/gateway-http-actix/src/management_resources/authorization_receipts.rs'
  row['evidence']=historical+'; '+api+'; '+device+'; '+pointer
  row['gap']='other channels/identity/batch import/concurrent publish and failure paths retain original coverage; no full story replay'
  if n in [1,3,5,6,7,9,10,11,12,13,14,16,17,18,19]:row['current_slice']='PASS: five API branches / two device recoveries / prior-draft pointer, as applicable; Kiro/Kimi only for device'
 elif n<=32:
  row['implementation']='web/prism/src/features/accounts/AccountModelsDialog.tsx; web/prism/src/features/upstreams/ProviderDialog.tsx; web/prism/src/features/models/connectModel.ts; crates/gateway-http-actix/src/management_resources.rs'
  row['evidence']=historical+'; '+api
  row['gap']='catalog failure/staleness/conflicts and explicit call evidence retain historical coverage; live Provider NOT_RUN'
  if n in [24,26,29,30]:row['current_slice']='PASS: explicit manual new model, no grants created; all preexisting group/key/grants unchanged; no inference by onboarding'
 elif n<=53:
  row['implementation']='web/prism/src/features/accounts/AccountsPage.tsx; web/prism/src/features/accounts/AccountBatchDialog.tsx; apps/gateway/src/runtime.rs; crates/gateway-store/src/control_plane.rs'
  row['evidence']=historical+'; '+device
  row['gap']='in-flight disable/delete/native failures/quota/Usage/ledger retain historical bounded coverage; full management operations and upgrade not replayed'
  if n==52:row['current_slice']='PASS: saved/unknown/expired/denied/failed/applied facts for focused device/API paths'
 elif n<=75:
  row['page_action']='public Chat/Messages/Responses JSON/SSE; multi-turn/tool; admission/stream/continuity/compact/WS'
  row['implementation']='apps/gateway/src/runtime/batch_c.rs; apps/gateway/src/runtime/batch_c/grok.rs; apps/gateway/src/runtime/batch_c/public_response.rs; crates/provider-grok/src/web_production.rs; crates/provider-kiro/src/conversation_request.rs'
  row['evidence']='historical combination ledger 2026-10-01 source 2e43ea4bccd7c1253df7d5ceddfbc3f4e4bc61fb; prior local HTTP evidence; current Full local tests'
  row['gap']='actual model/account/route/declarations not frozen; all promised live combinations blocked by authorization; deterministic refusal is not support'
  if n in [54,55,60,61]:
   row['status']='BLOCKED';row['gap']+='; Web native multi-turn missing and Kiro hard output cap unimplemented'
  if n==60:row['accepted_limit']='Grok Web tools UNSUPPORTED_ACCEPTED; text/multi-turn remains promised'
  if n==73:row['status']='BLOCKED';row['gap']+='; E-05 requires explicit user decision; no rule changed'
  if n==75:row['accepted_limit']='Messages exact token count UNSUPPORTED_ACCEPTED; do not invent estimates'
 elif n<=81:
  row['page_action']='desktop pointer/wheel/full-diff review and apply; remaining page inventory/themes/narrow/states'
  row['implementation']='web/prism/src/design/workspace.css; web/prism/src/components/InlineWorkspace.tsx; web/prism/src/features/config-versions/ConfigurationTaskReview.tsx; web/prism/src/app/app.css'
  row['evidence']=historical+'; '+pointer+'; initialize-pointer-diagnostic.json'
  row['current_slice']='PASS: 1280x800 / 1440x900 old-draft pointer actions; initial mandatory-password pointer retry independently recorded'
  row['gap']='whole-site page/state/theme/narrow inventory NOT_RUN this batch; initial login click interception was not reproduced reliably and is retained as follow-up'
 else:
  row['page_action']='source/receipt/matrix/review/compatibility/candidate and deployment gates'
  row['implementation']='scripts/check.sh; scripts/acceptance/cpar-api-actual-matrix.mjs; scripts/acceptance/cpar-device-actual-ui.mjs; docs/reports/cpar-continuity-operations-local-20261009.md'
  row['evidence']='evidence-index.json; source-binding.json; full-final-corrected-receipt.json; independent review attestation; historical originals'
  row['current_slice']='local scope recorded with exact versions; independent gates remain separate'
  row['gap']='#57 overall incomplete; full upgrade/site/live combinations and candidate/CI/deploy/production not accepted'
# Give the focused witnesses exact actions and artifacts; a witness does not close its parent story.
focused = {
 1: ('AI 提供商 → 添加 → 填表 → 保存并接入', 'PASS: 五 API × 三匹配分支；编辑既有配置未在本次全量重放', api),
 3: ('OAuth → Kiro/Kimi 设备授权 → 开始', 'PASS: 两渠道实际入口、授权挑战；其他 OAuth 渠道 NOT_RUN', device),
 5: ('AI 提供商 → Kimi API → 零匹配保存并接入', 'PASS: 空匹配创建连接并绑定；合成地址与材料', api+' / api-kimi-matching-0-save-bind-apply'),
 6: ('AI 提供商 → 五预设 → 各三匹配分支', 'PASS: Responses/Chat/Messages/Kimi API/Grok API 共十五分支', api),
 7: ('添加 → 选择预设 → 地址/协议 → 校验并应用', 'PASS: Grok 固定组件与兼容地址输入；真实渠道地址调用 NOT_RUN', api),
 9: ('添加 → 零匹配自动创建／唯一匹配自动复用', 'PASS: 五预设各两分支；创建数量与 owner/endpoint 精确核对', api+' / matching-0, matching-1'),
 10: ('添加 → 多匹配 → 未选择保存 → 选择后保存', 'PASS: 五预设，选择前零 POST，选择后精确绑定已有 owner', api+' / matching-2'),
 11: ('添加 → 单表单填写 → 保存并接入', 'PASS: 五 API 预设表单提交与应用', api),
 12: ('授权已提交但响应丢失 → 关闭 → 账号页 → 恢复原任务', 'PASS: 两渠道登记及返回读取；正常成功自动返回未单独覆盖', device),
 13: ('无旧草稿 → 保存并接入 → 自动校验/应用', 'PASS: 十五 API 分支保存、绑定、应用与 active readback', api),
 14: ('继续旧草稿 → 接入 → 核对十八项旧修改和完整清单 → 指针应用', 'PASS: 两桌面视口保存先不发布、完整核对后一次应用', pointer),
 16: ('授权／接入结果 → 查看保存、连接、待应用／已应用状态', 'PASS: 合成 API/device 丢响应及边界的分层反馈', api+'; '+device+'; '+pointer),
 17: ('已保存但校验／应用失败 → 就地恢复', 'NOT_RUN: 最终源码未新增完整校验/应用失败场景；上批保留原范围', historical),
 18: ('响应丢失 → 关闭／切页 → 原任务 → 读回执及原账号', 'PASS: 两设备渠道，不重放成功授权交换；API 丢响应仍沿用历史范围', device),
 19: ('未知结果关闭 → 离开／返回 → 恢复 → 连接／应用', 'PASS: 两设备渠道，原账号保存，不删除、不取消、不重复创建', device),
 24: ('添加 → 手动新模型 → 保存并接入', 'PASS: 接入不要求创建 grants', api),
 26: ('复用停用连接 → 手动新模型 → 应用 → 读 permissions/keys/groups', 'PASS: 旧权限与停用状态保持，新模型零 grants', api),
 29: ('添加 → 手动输入新模型 → 保存并接入', 'PASS: 手动补充模型；映射冲突/所有目录状态 NOT_RUN 本批', api),
 30: ('保存／连接／应用 → 合成服务收件核对', 'PASS: onboarding 无推理 POST；主动真实测试分支 NOT_RUN', 'final-persistence-verification.json.gz; '+api),
 52: ('设备 denied/expired/failed/TTL/transport/pending_then_success', 'PASS: 两渠道各六边界准确状态和数据库结果；其他管理状态未全量重放', 'final-device-boundaries.json.gz'),
 79: ('1280×800／1440×900 → 旧草稿 → 指针／滚轮 → 核对／应用', 'PASS: 两桌面视口指针可达；窄屏/主题/全键盘 NOT_RUN 本批', pointer),
 81: ('接入／设备流程关闭、离开、返回、保存、核对、应用', 'PASS: 聚焦流程指针及恢复；全站搜索/筛选未全量重放', pointer+'; '+device)
}
for row in matrix['stories']:
 n=int(row['id'][2:])
 if n in focused: row['page_action'],row['current_slice'],row['evidence']=focused[n]
 elif n in [76,77,78,80]:
  row['current_slice']='NOT_RUN: 两桌面视口局部证据不能证明全站视觉、状态或高级入口'
  row['evidence']=historical+'; '+pointer+' (related witness only)'
 if n in focused and n<=32:
  row['implementation']='web/prism/src/features/upstreams/ProviderDialog.tsx:47,65,122,136; web/prism/src/features/upstreams/apiProviderModel.ts:6; web/prism/src/features/upstreams/connectionPresets.ts; web/prism/src/features/accounts/KiroDeviceDialog.tsx:33,61; web/prism/src/features/accounts/KimiDeviceDialog.tsx:77,208; web/prism/src/features/config-versions/ConfigurationTaskReview.tsx:17,25; crates/gateway-http-actix/src/management_resources/authorization_receipts.rs'
for row in matrix['acceptance_tests']:
 line=next(line for line in issue9['body'].splitlines() if line.startswith('| '+row['id']+' '))
 row['requirement']=line.split('|')[2].strip()
 row['current_slice']='PASS within focused API/device/desktop slice' if row['id'] in ['T01','T02','T03','T04','T05','T13'] else 'historical bounded evidence, current local Full; not full test-case acceptance'
 row['evidence']='evidence-index.json; historical originals'
 row['missing_layer']={'T01':'remaining channels and material entry points','T02':'whole identity/dedup conflicts not replayed','T03':'remaining concurrent revision/native publish failures','T04':'other callback/delete/restore/batch boundaries','T05':'remaining catalog states/public permissions/channel-specific paths','T06':'whole evidence invalidation/observation states; live Provider','T07':'all in-flight/native actions actual page','T08':'Web text multi-turn/Kiro hard cap; exact real combinations','T09':'all actual declared extensions plus live exact semantics','T10':'all channel modes and live stream/cancel counters','T11':'E-05 explicit decision; actual declarations/live store/WS','T12':'whole installed-existing-data migration/rollback acceptance','T13':'full page/state/theme/narrow inventory; intermittent login interception','T14':'explicit real-account/request budget/environment authorization','T15':'candidate/artifact/remote Phase/CI/deploy/production'}[row['id']]
for row in matrix['baseline_findings']:
 row['requirement']=next(line.split('|')[2].strip() for line in issue9['body'].splitlines() if line.startswith('| '+row['id']+' '))
 row['current_slice']='focused actual page evidence added; original broader condition not silently closed'
 row['evidence']='evidence-index.json; historical originals'
 row['missing_layer']={'F01':'remaining account/channel-specific entry points','F02':'other preparation/persistence failure branches','F03':'other OAuth callback/actor/TTL branches; real Provider','F04':'remaining native/global/quota and concurrency paths','F05':'Web multi-turn and Kiro native hard output cap; real combinations','F06':'rich metadata not representable; actual declarations/live evidence','F07':'all real Provider combinations outside authorization','F08':'whole legacy ADR/selector/fixture inventory not replayed; E-05 pending'}[row['id']]
for row in matrix['issue57_clauses']:
 row['status']='PARTIAL';row['evidence']='issue57-matrix.json; evidence-index.json; source-binding.json; historical originals'
 row['missing_layer']={'AC01':'198 基础组合与实际扩展声明尚未真实验收；全站及升级条款仍 PARTIAL','AC02':'聚焦停用/权限/连接保持已验证；全量既有数据迁移、Usage/账本/历史回退未重放','AC03':'本批 clean Full/页面/独立评审单独绑定；其他批次 Phase 沿用各自原回执，不扩大','AC04':'E-05、Web 多轮、Kiro 硬上限及真实必需证据缺口保持未完成','AC05':'真实账户/环境/授权/预算 BLOCKED；整体未关闭','AC06':'冻结新候选前置条件未满足；制品/签名/影响/回退与部署 NOT_RUN','AC07':'本批交付与后续项已记录；#9/#57 原范围未调整或关闭','AC08':'本批实际页面和 loopback 独立 PASS；真实 Provider/生产 NOT_RUN','AC09':'本批无 OpenAPI 变化，生成客户端不手改；跨边界 FYI 与账号/权限检查留证','AC10':'本批固定源 clean Full 和范围提交；外部动作均未授权/未执行'}[row['id']]
matrix['historical_combination_ledger']['reuse']='original execution time/source/status preserved; current rows below are a separate authorization audit'
matrix['current_live_combinations']=copy.deepcopy(history['basic_new_build_combinations'])
for row in matrix['current_live_combinations']:
 row['source_sha']=source;row['historical_status']=row['status'];row['status']='BLOCKED';row['observed_at_utc']=None
 row['blockers']=['REAL_PROVIDER_NOT_AUTHORIZED','EXACT_CURRENT_MODEL_ACCOUNT_ROUTE_NOT_FROZEN']
 if row['channel']=='grok.web':
  if row['scenario']=='text_multi_turn':row['blockers'].append('SOFTWARE_WEB_MULTITURN_MISSING')
  else:row['support']='UNSUPPORTED_ACCEPTED';row['applicable']=False;row['status']='NOT_RUN';row['blockers']=[]
matrix['historical_kiro_cap_rows']=history['kiro_explicit_output_limit_combinations']
matrix['historical_rich_metadata_rows']=history['conditional_rich_metadata_combinations']
matrix['historical_extension_readiness']=history['extension_readiness']
matrix['capability_notes']=[{'capability':'Grok Web native multi-turn','status':'BLOCKED','evidence':'web_production.rs:312/329; runtime/batch_c/grok.rs:478','current':'FAIL_CLOSED; not supported'}, {'capability':'Kiro hard output cap','status':'BLOCKED','evidence':'conversation_request.rs:309; runtime/batch_c.rs:393/474','current':'FAIL_CLOSED; native hard cap missing'}, {'capability':'Grok ordinary JSON role/Usage','status':'PARTIAL','evidence':'build_responses.rs:2108/2338; runtime/batch_c/grok.rs:207/306','current':'ordinary bridge supported locally; do not copy stale ordinary-JSON blocker; rich non-equivalent metadata still refuses'}]
(assets/'issue57-matrix.json').write_text(json.dumps(matrix,ensure_ascii=False,indent=2)+'\n')
def table(rows,columns):
 return '\n'.join(['| '+' | '.join(title for key,title in columns)+' |','| '+' | '.join('---' for _ in columns)+' |']+['| '+' | '.join(str(row.get(key,'')).replace('|',' / ').replace('\n',' ') for key,title in columns)+' |' for row in rows])
body=['# CPAR-48 / #57 条款与页面动作矩阵（2026-10-09）','',f'固定 base `01dae81bb9778c92d7e76e49bbed14c3fdc565d7`；本批实际运行源码 `{source}`。总体 **PARTIAL**。本表逐项列出原 88 条故事、F01–F08、T01–T15 和 #57 十条验收要求；局部实际页面 PASS 不关闭整条承诺。','', '[机器可核查完整矩阵](assets/cpar-continuity-operations-20261009-171952/issue57-matrix.json) 包含独立的当前 198 个基础组合行，以及历史 24 个 Kiro 上限、12 个丰富 metadata 和 49 个扩展准备行。历史行的源码、时间与状态保留；新行是当前授权核对，真实执行数仍为零。扩展 actual_declared=null 不等于未声明，必须在真实组合验收前冻结实际声明。','', '## 本批实际页面证据','', 'API 5×3、设备提交丢响应恢复、六类设备边界与两个桌面视口均指向独立原始 JSON、命令回执和截图，见 [证据索引](assets/cpar-continuity-operations-20261009-171952/evidence-index.json)。页面源于 stock serve；设备为进程专用 loopback 网络，保留生产固定 URL、SNI 和 TLS 校验，不是真实 Provider。原 2026-10-01 和上一批 2026-10-09 执行保持原来源，不重标为本次。','', '## 用户故事','',table(matrix['stories'],[('id','条款'),('requirement','原要求'),('page_action','页面／动作'),('implementation','实现位置'),('current_slice','本批证据范围'),('evidence','证据'),('status','整条结论'),('gap','剩余缺口')]),'', '## F01–F08','',table(matrix['baseline_findings'],[('id','条款'),('requirement','原要求'),('status','结论'),('current_slice','本批范围'),('evidence','证据'),('missing_layer','缺失层')]),'', '## T01–T15','',table(matrix['acceptance_tests'],[('id','条款'),('requirement','原要求'),('page_action','页面／动作'),('implementation','实现位置'),('status','整条结论'),('current_slice','本批证据范围'),('missing_layer','缺失层')]),'', '## #57 验收要求','',table(matrix['issue57_clauses'],[('id','条款'),('requirement','原要求'),('page_action','页面／动作'),('implementation','实现位置'),('status','结论'),('missing_layer','剩余门槛')]),'', '## 11 渠道 × 三协议','', '| 渠道 | Chat Completions | Messages | Responses | 实际执行及限制 |','| --- | --- | --- | --- | --- |']
for channel in sorted({r['channel'] for r in matrix['current_live_combinations']}):body.append(f'| {channel} | BLOCKED | BLOCKED | BLOCKED | 每协议 JSON/SSE × 文本多轮／单工具／连续工具。真实执行 0；'+('Web 文本多轮软件缺口，工具明确不支持；' if channel=='grok.web' else 'Kiro native 硬上限另列；' if channel=='kiro' else '')+'模型／参数／账号／配置／当前声明未冻结，缺真实授权 |')
body+=['', '## 后续范围与明确限制','', '- Grok Web 工具、Messages 精确 token 计数保持已接受“不支持”；不存在新豁免。Web 原生多轮与 Kiro 硬上限仍是软件缺口，拒绝正确不代表能力支持。','- E-05 仍待用户裁定，#36 的无例外 revision 条款与 BC-RESP-003 的严格同授权刷新例外未擅自统一。','- 全站主题／窄屏／状态清单、完整既有数据升级／回退、真实渠道、远端 CI、新候选制品、部署和生产不以本批页面 PASS 代替。','- 合成 AccessGroup 准备重用了唯一 name/id 时返回 500；保留原失败，合法输入改为运行专用命名。重复资源错误分类列后续，不扩大本批改动。','- 首次强制改密指针暂态拦截未稳定复现；第二次同源指针操作通过，保留两份结果，不声明布局缺陷已修复。','- 旧 Grok API 非规范组件不自动迁移或启用；必要时由操作者核对固定规范组件与已有关联。']
(root/'docs/reports/cpar-continuity-issue57-matrix-20261009.md').write_text('\n'.join(body)+'\n')
print({'stories':len(matrix['stories']),'F':len(matrix['baseline_findings']),'T':len(matrix['acceptance_tests']),'AC':len(matrix['issue57_clauses']),'live_rows':len(matrix['current_live_combinations']),'live_executed':sum(row['executed'] for row in matrix['current_live_combinations'])})
