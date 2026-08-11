// zh-CN is the source of truth: `Pack` is derived from this object, so adding a
// key here without translating it in en.ts is a type error rather than a silent
// fallback to Chinese text. Pack lives here rather than in messages.ts so the
// two packs do not have to import from each other's module.
export const zh = {
  appTitle: "Prism · 网关管理",
  unlock: {
    title: "解锁管理面板",
    managementKey: "Management Key",
    managementKeyHint: "mgmt_ 前缀,32-512 位字母数字或 _ -;可直接从服务器日志粘贴,换行与引号会自动清理。",
    csrfToken: "CSRF Token",
    csrfTokenHint: "csrf_ 前缀;浏览器部署必填,本机 CLI 访问可留空。",
    revealToggle: "显示密钥",
    submit: "解锁",
    fillDemo: "填入演示密钥(fixture 模式)",
    hint: "密钥仅存于本页内存,刷新后需要重新输入。",
    failed: "管理访问不可用 —— 请检查密钥、网络位置与部署配置。",
    invalidShape: "格式不符:Management Key 需为 mgmt_ 前缀,32-512 位字母数字或 _ -。",
  },
  nav: {
    overview: "总览",
    usage: "用量分析",
    monitoring: "请求监控",
    versions: "配置版本",
    upstreams: "上游",
    models: "模型与路由",
    access: "访问控制",
    egress: "出口策略",
    runtime: "运行时",
    audit: "审计与备份",
    settings: "设置",
  },
  version: {
    none: "未选择版本",
    conflict: "配置已被其他会话修改,已刷新数据 —— 请确认后重试。",
    conflictAck: "知道了",
    readOnly: "当前版本只读(非草稿)。",
    pickerLabel: "配置版本",
  },
  state: {
    empty: "暂无数据",
    filteredEmpty: "没有符合过滤条件的结果",
    unavailable: "此部署未启用该运行时投影",
    unwired: "事件管道尚未接线(G2)—— 观测数据在后端接线后出现",
  },
  settings: {
    title: "设置",
    lead: "本页只做本会话内的事。网关没有 settings 端点,面板也不写任何浏览器存储,所以这里的每一项都在刷新后回到默认值 —— 这是刻意的,不是缺失。",

    appearance: "外观",
    appearanceHelp: "默认跟随系统。显式选择只影响本标签页,刷新即失效。",
    themeSystem: "跟随系统",
    themeLight: "浅色",
    themeDark: "深色",
    themeActive: "当前生效",

    language: "语言",
    languageHelp: "同样只存于内存。界面文案立即切换,后端返回的枚举与标识符不翻译(它们是契约的一部分)。",

    session: "会话",
    sessionHelp: "Management Key 与 CSRF Token 只存在于内存,从不落盘、从不进 URL。",
    sessionKeyLabel: "Management Key",
    sessionCsrfLabel: "CSRF Token",
    sessionCsrfAbsent: "未提供(本机 CLI 访问可留空)",
    lock: "锁定并清除密钥",
    lockHelp: "立即清空内存中的密钥并退回解锁页。离开这台机器前用它。",

    render: "渲染能力",
    renderHelp: "运行时探测结果,不是开关 —— 用于解释这台浏览器上的玻璃为什么长这样。",
    lensOn: "真实折射(Chromium)",
    lensOff: "分层兜底(Firefox / Safari)",
    lensExplain: "Firefox 与 Safari 会解析 backdrop-filter: url() 却什么都不画,所以由探测结果决定走哪条路径。",
    prefReduceMotion: "减弱动效",
    prefReduceTransparency: "降低透明度",
    prefMoreContrast: "提高对比度",
    prefOn: "已开启",
    prefOff: "未开启",
    prefHelp: "全部来自系统设置,面板只服从。开启后玻璃会依次退化为半透明、纯实心。",

    build: "构建",
    buildMode: "运行模式",
    buildModeDev: "开发(fixture 后端可用)",
    buildModeProd: "生产",
    buildFixtures: "Fixture 后端",
    buildFixturesOn: "已启用 —— 数据是本地伪造的,不是真实网关",
    buildFixturesOff: "未启用 —— 请求发往真实网关",
    contract: "契约",
  },
} as const;

/** The shape both packs must satisfy: the same keys, with every string leaf
 *  widened to `string`. Without the widening, `as const` pins each value to its
 *  own literal type and no translation can satisfy it. Recursive because the
 *  pack mixes top-level strings (appTitle) with grouped ones. */
type Widen<T> = T extends string ? string : { readonly [K in keyof T]: Widen<T[K]> };
export type Pack = Widen<typeof zh>;
