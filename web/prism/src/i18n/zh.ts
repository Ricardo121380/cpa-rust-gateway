// zh-CN is the source of truth: `Pack` is derived from this object, so adding a
// key here without translating it in en.ts is a type error rather than a silent
// fallback to Chinese text. Pack lives here rather than in messages.ts so the
// two packs do not have to import from each other's module.
export const zh = {
  appTitle: "Prism · 网关管理",
  unlock: {
    title: "管理员登录",
    username: "账号",
    password: "密码",
    showPassword: "显示密码",
    hidePassword: "隐藏密码",
    submit: "登录",
    busy: "请稍候…",
    required: "请输入账号和密码",
    failed: "账号或密码不正确",
    unavailable: "暂时无法登录，请稍后重试",
    rateLimited: "尝试过于频繁，请一分钟后重试",
    changeTitle: "设置新密码",
    firstChange: "首次登录，请设置你的新密码。",
    initialPassword: "初始密码",
    currentPassword: "当前密码",
    newPassword: "新密码",
    confirmPassword: "确认新密码",
    passwordPlaceholder: "至少 12 个字符",
    passwordPolicy: "请使用与原密码不同的 12–128 个字符",
    passwordMismatch: "两次输入的新密码不一致",
    currentPasswordWrong: "当前密码不正确",
    savePassword: "保存并重新登录",
    passwordChanged: "密码已更新，请重新登录",
    back: "返回登录",
    revealToggle: "显示密钥",
  },
  nav: {
    overview: "总览",
    usage: "用量分析",
    monitoring: "请求与失败",
    billing: "计费与价格",
    versions: "配置版本",
    upstreams: "上游",
    models: "模型与路由",
    access: "访问控制",
    egress: "出口策略",
    runtime: "运行诊断",
    accounts: "账号池",
    catalog: "模型目录",
    audit: "审计与备份",
    settings: "设置",
  },
  navigation: {
    operations: "运行", resources: "资源", management: "管理",
    menu: "打开全部栏目", search: "搜索栏目", theme: "切换深浅外观",
  },
  version: {
    none: "未选择版本",
    conflict: "配置已被其他会话修改,已刷新数据 —— 请确认后重试。",
    conflictAck: "知道了",
    readOnly: "当前版本只读(非草稿)。",
    pickerLabel: "配置版本",
  },
  state: {
    loading: "读取中…", readFailed: "读取失败", previousData: "下面保留上次读取的结果，并非最新状态。", retry: "重试读取",
    empty: "暂无数据",
    filteredEmpty: "没有符合过滤条件的结果",
    unavailable: "此部署未启用该运行时投影",
    unwired: "事件管道尚未接线(G2)—— 观测数据在后端接线后出现",
  },
  settings: {
    title: "设置",
    lead: "外观与语言偏好仅用于当前会话。",

    appearance: "外观",
    appearanceHelp: "默认跟随系统。显式选择只影响本标签页,刷新即失效。",
    themeSystem: "跟随系统",
    themeLight: "浅色",
    themeDark: "深色",
    themeActive: "当前生效",

    language: "语言",
    languageHelp: "同样只存于内存。后端返回的标识符不翻译 —— 它们是契约的一部分。",
    languageCoverage:
      "部分页面仍以中文显示。",

    session: "会话",
    sessionHelp: "刷新或退出后需重新登录。",
    sessionKeyLabel: "管理员",
    sessionCsrfLabel: "会话到期",
    sessionCsrfAbsent: "—",
    lock: "退出登录",
    lockHelp: "",

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
