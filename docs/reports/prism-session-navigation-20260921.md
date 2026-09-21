# 会话撤销与导航专项复核 — 2026-09-21

## 结论

EgoLite 对真实本地 gateway 的三个受控时序全部通过；同一标签页跨构建加载通过。本批没有复现前次空画布，无法确认其根因，**不将“未复现”写为“已修复”**，也未按猜测修改产品鉴权/路由。现有产品代码仍为上一批版本。

用户本轮明确浏览器测试使用 EgoLite，不使用 Safari。已停止 Safari 测试，仅关闭本次创建的测试窗口；既有用户窗口保留。历史 Safari 证据不删除，但不作为当前批次验收要求或当前通过证据。

## 运行边界

- 代码检查基线：`4edff0a`；嵌入资源 revision `c628bc48d1c4633777b6fdfb`。
- 本地管理监听：127.0.0.1:55380，前次独立合成状态，真实管理登录/撤销/查询接口。
- 合成密码只从私有 qa-password 文件读入；撤销所需会话只保留在页面闭包，输出不含密码、令牌、请求头或响应体。
- 没有生产部署、真实 Provider 推理、账号授权、配置发布或历史数据删除。

## 时序与结果

| 场景 | 控制点 | 结果 |
|---|---|---|
| immediate | 登录后管理画布出现，立即撤销并跳转设置 | 撤销204；真实受保护接口404；登录表单可见；无管理画布/JS异常 |
| settled | 等配置上下文出现后撤销，再跳转设置 | 同上 |
| held | 撤销后暂缓把真实404响应交给前端，导航到设置后释放 | 同上；没有伪造接口响应，仅控制传输交付时序 |

三个场景分别有[原始脱敏事件](evidence/prism-session-navigation-20260921/)，包括 popstate/hashchange、请求路径、HTTP状态、取消和释放时间。最终脚本运行3/3通过，不将前期探测重复计数。

同标签页加载旧 `af89c2c` 前端构建后，切换为当前四文件并正常刷新，不清除缓存。旧 revision `7fd29ab79943be05b928faea`，新 revision `c628bc48d1c4633777b6fdfb`。main/vendor/CSS 均引用新 revision，登录表单存在。此次 transferSize 均为0，说明复用了此前缓存的新版本资源；**不能说重新从服务器下载了资源**。此受控静态服务仅验证资源加载，不代替部署切换或真实网关鉴权测试。见 [asset-switch.json](evidence/prism-session-navigation-20260921/asset-switch.json)。

## 复跑脚本

新增 `scripts/acceptance/prism-session-navigation.mjs`。使用 EgoLite 的 Node 运行时，将以下非秘密配置对象置于脚本内容之前，并把组合后的源码通过 stdin 交给 `ego-browser nodejs`：

```js
globalThis.PRISM_SESSION_QA = {
  spaceId: /* 当前任务拥有的活动空间编号 */,
  root: /* 独立合成状态目录，含0700/0600保护的qa-password */,
  origin: "http://127.0.0.1:55380",
  mode: "immediate", // 或 settled / held
  output: "/tmp/prism-session-result.json"
};
```

脚本拒绝非loopback origin，要求密码文件不可被组/其他用户读取；不创建/关闭空间，不自动重试。调用者负责启动独立网关并最终关闭空间。该文件不能直接用普通Node运行，因为 taskSpace 是 EgoLite 提供的API。EgoLite运行时不继承普通shell环境变量，参数通过非秘密源码对象传递。

`node --check` 通过，脚本三个模式均经真实运行。无产品代码修改，本批未重复前次已通过的产品构建和Rust门禁。

## 未解决事项与发布判断

最初空画布发生时未保存错误和导航事件，现有结果不能追溯证明它属于自动化还是应用竞态。保留为未归因验收风险，发布前继续记录这一限定路径，出现时以本脚本回执为定位入口；不以无依据的鉴权放宽、重放写入或全页强制刷新掩盖它。整体计划与发布门禁未标完成，本批未上线。
