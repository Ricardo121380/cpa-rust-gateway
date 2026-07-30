# DESIGN.md · Prism 已建成的视觉世界

> 本文档记录 **已经存在的东西**,不是意图。基准是已批准的 `5df23a7`(Liquid Glass:让材质真的读作玻璃)。
> 每一个数值要么来自 `src/design/tokens.css` / `src/design/glass.css` / `src/app/app.css` 的字面量,
> 要么来自在 `127.0.0.1:5173`(fixture 模式,Chromium 1440×900 @2x,双主题)对运行中面板的实测。
> 契约文档是 `cpa-rust-gateway/docs/07-management-frontend-design.md`;**凡本文与契约数值不一致处,以本文为准并在 §10 记录原因**。
>
> 权威文件:
> `src/design/tokens.css`(环境与材质变量,三层主题) ·
> `src/design/glass.css`(`.glass` 配方) ·
> `src/components/glass/PrismLens.tsx`(SVG 透镜、能力探测、`data-over` 探针) ·
> `src/components/glass/GlassSurface.tsx`(唯一玻璃原语 + 面数预算) ·
> `src/app/app.css` + `src/app/AppShell.tsx`(壳体几何、滚动边缘)。

---

## 1. 这个世界是什么

一层**永不滚动的彩色环境**,上面浮着**最多三面功能玻璃**(顶栏 / 侧栏 / 草稿坞),
中间夹着**一个整屏滚动的实心内容画布**。内容从玻璃**底下穿过**——这是整套视觉唯一的支点:
玻璃之所以读作玻璃,不是因为它自己长得像玻璃,而是因为它背后有东西在动。

数据永远待在实心面上。玻璃只承载导航与状态,不承载数字。

---

## 2. 环境层(ambient)

### 2.1 由什么构成

`AppShell` 在 `.shell` 内铺两个 `position: fixed` 的装饰层(均 `aria-hidden`,`pointer-events: none`):

| 层 | 元素 | 内容 |
|---|---|---|
| 0a | `.ambient` | `--canvas` 底色 + 5 层 authored gradient |
| 0b | `.ambient-grain` | 一个 `<rect>` 填 `filter: url(#prism-grain)` 的内联 SVG |

`.ambient` 的 5 层,自上而下(CSS `background-image` 顺序):

1. **棱镜光带** `linear-gradient(107deg, …)` —— 5 个色相站点(`#ff5f6d` `#ffc371` `#6ee7b7` `#60a5fa` `#a78bfa`)挤在 12%–32% 之间,每站点透明度 = `--amb-streak × (22…30)%`,在 `oklab` 里混合;
2–5. **四盏彩色光** `radial-gradient`:`64%×52% at 4% -6%`(冷主光)、`56%×50% at 98% 2%`(紫补光)、`70%×62% at 36% 108%`(青反弹)、`34%×40% at 90% 62%`(暖轮廓);
6. **中尺度光泽** `conic-gradient(from 205deg at 24% 22%, …)`,让场永远不局部平坦。

`.ambient-grain` 的噪点:`feTurbulence type="fractalNoise" baseFrequency="0.82" numOctaves="3" seed="7" stitchTiles="stitch"` → 去饱和 → `feFuncA slope="0.42"`。
混合模式:亮色 `overlay`,暗色 `soft-light`(由 `[data-theme="dark"]` 与 `prefers-color-scheme` 两条规则各写一次)。

### 2.2 为什么必须有**小尺度色相变化**

这是整个环境层存在的唯一理由,写在 `tokens.css` 的注释里:

> 折射揭示的是**小空间尺度上的色相变化**。四盏宽 radial 光单独存在时读作一片平的粉彩底 —— 实测结果是侧栏看起来像一张卡片。

同一条推论解释了噪点层:模糊和折射都需要高频细节才能"咬住"。
也解释了为什么 `--canvas-ambient`(一条 160° 线性渐变)**不足以**当背景:频率太低,折射它等于折射一片纯色。

### 2.3 数值(实测生效值)

| 变量 | 亮色 | 暗色 |
|---|---|---|
| `--amb-1` 冷主光 / 强度 | `#5f9dff` / `0.42` | `#1e5fe0` / `0.34` |
| `--amb-2` 紫补光 / 强度 | `#b98cff` / `0.38` | `#7c3aed` / `0.30` |
| `--amb-3` 青反弹 / 强度 | `#3fd7bd` / `0.34` | `#0f9d8f` / `0.26` |
| `--amb-4` 暖轮廓 / 强度 | `#ffb877` / `0.26` | `#c2560a` / `0.20` |
| `--amb-streak` 棱镜光带 | `0.32` | `0.26` |
| `--amb-grain` 噪点 | `0.40` | `0.45` |
| `--canvas` 底色 | `#f2f2f7` | `#0b0b10` |

无图片、无 CDN、无 `<style>`:全部是 authored CSS 与内联 SVG,CSP `style-src 'self'` 下成立。

---

## 3. 玻璃配方(`.glass`)

### 3.1 层序(自下而上)

```
backdrop-filter    透镜(处理 C)或 blur+saturate+brightness+contrast(处理 B)
::after  z-index 0 宽边缘辉光(玻璃"体"接到的光)
background-image   垂直 tint 渐变(不是平铺 rgba)
content  z-index 1
::before z-index 2 高光环(遮罩梯度描边,不是 1px border)
box-shadow         四段外阴影 + 四段内阴影
```

`.glass` 自身 `border: 0; background-color: transparent; isolation: isolate;`,
`.glass > *` 强制 `position: relative; z-index: 1`,以保证内容夹在辉光与高光环之间。

### 3.2 玻璃体(处理 B —— 同时是 C 的兜底)

| | 亮色 | 暗色 |
|---|---|---|
| `--glass-blur` | `12px` | `13px` |
| `--glass-sat` | `180%` | `165%` |
| `--glass-bright` | `1.06` | `1.04` |
| `--glass-contrast` | `1.02` | `1.04` |

暗色的原则写在注释里:**更低饱和、更高亮度** —— 暗面压在暗底上会消失,除非它把背景**抬起来**。
(注意:这条注释是"抬起来";但实际调参后暗面用的是**熏黑**的 tint 渐变,见 §10.5。)

### 3.3 垂直 tint 渐变(不是平铺色)

`linear-gradient(to bottom, top 0%, mid 46%, bot 100%)`:

| | 亮色 | 暗色 |
|---|---|---|
| top | `rgba(255,255,255,.62)` | `rgba(12,12,18,.52)` |
| mid | `rgba(255,255,255,.44)` | `rgba(12,12,18,.40)` |
| bot | `rgba(255,255,255,.52)` | `rgba(12,12,18,.48)` |

理由(注释原文):一片平铺 `rgba()` 玻璃体是旧基线读作卡片的**首要原因** —— 真实玻璃从不只有一个值,光从上方来,顶边承载更多。
底边比中段略重,是台面反光。

### 3.4 高光环 `::before`

不是 1px 描边,而是 `padding: var(--ring-w)` + `mask-composite: exclude` 把一条 `147deg` 梯度画进 border box:
5 个站点(0% / 16% / 38% / 60% / 84% / 100%),10 点钟与 4 点钟方向亮,两侧腰部熄灭到近乎 0。

| 站点 | 亮色 | 暗色 |
|---|---|---|
| `--ring-1` (0%) | `rgba(255,255,255,.95)` | `rgba(255,255,255,.62)` |
| `--ring-2` (16%) | `.42` | `.20` |
| `--ring-3` (38–60%) | `.02` | `.02` |
| `--ring-4` (84%) | `.55` | `.24` |
| `--ring-5` (100%) | `.90` | `.50` |
| `--ring-w` | `1px` | `1px` |

### 3.5 边缘辉光 `::after` 与内辉光

`::after` 是两条窄带:顶部 `--glass-edge-top` 渐隐到 `--glass-edge-top-h`(默认 `18px`),
底部 `--glass-edge-bot` 渐隐到 `--glass-edge-bot-h`(默认 `14px`)。
高度参数化是为了让模态层能加宽(更厚的板有更宽的受光边),**而不必覆写伪元素本身** —— 覆写会盖过 §8 的降级规则。

`--glass-edge-top / bot`:亮色 `.28 / .14`,暗色 `.10 / .05`。

内辉光 `--glass-inner` 是四条 inset:两条 1px 边线 + 两条软内部渐变。
亮色 `.90 / .45 / .95 / .60`,暗色 `.30 / .16 / .55 / .35`。

### 3.6 阴影:四段 + `data-over` 增强

```
--glass-shadow: 接触(0 1px 1px) + 近场(-3px spread) + 中场(-8px spread) + 环境(-22px spread)
```

注释原文:**四段带负 spread** 才读作"浮在上面";一坨 32px 的 blob 不行。

`PrismLens` 每面每滚动帧做一次 `elementFromPoint` 探针,写 `data-over="content" | "ambient"`;
`content` 时切到 `--glass-shadow-boost`(各段透明度整体上抬 ~40%)。这是 WWDC25 的自适应阴影行为。
实测:静止在总览页时,顶栏与侧栏的 `data-over` 均为 `content`。

### 3.7 处理 C:真折射(`PrismLens.tsx`)

C 就是 B 把 `backdrop-filter` 换成 SVG 滤镜,**其余层完全共用**,所以退回 B 只是一个属性。

滤镜链:`feImage`(位移图)→ `feColorMatrix`(B 通道→alpha,得到斜面遮罩)→
`feGaussianBlur(frost)` 得到磨砂内部 → `feGaussianBlur(rimBlur)` + `feDisplacementMap` + `feComposite in` 得到折射边环 →
`feMerge` → `feColorMatrix saturate` → `feComponentTransfer` 线性提亮 1.06。

位移图在 `<canvas>` 里逐像素算,导出成 `data:` URL(CSP `img-src 'self' data:` 允许):

- 剖面用 **Apple squircle** `y = (1-(1-x)^4)^(1/4)`,不是圆弧 —— 圆弧在被拉成 1416px 宽的顶栏后,斜面与平坦内部交界处会出现**可见硬缝**;
- 沿半径做 **Snell–Descartes** 折射(空气 n=1 → 玻璃 n=`--lens-ior`),200 个采样;
- 通道:R = x 位移,G = y 位移,B = 斜面覆盖度(遮罩搭同一张图,免费);
- `feDisplacementMap` 的 `scale` 取**负值** = 向外采样,这是物理正确的凸透镜。

几何参数(`--lens-*`,可被单面覆写):

| 参数 | 值 | 含义 |
|---|---|---|
| `--lens-bezel` | `26` | 自边缘向内量的斜面带宽度(px) |
| `--lens-thickness` | `34` | 虚拟玻璃厚度(px),决定折射强度 |
| `--lens-ior` | `1.5` | 折射率 |
| `--lens-gain` | `1` | 艺术倍率 |
| `--lens-rim-blur` | `3` | 位移**之前**的预模糊 |

**斜面必须夹到面的短边。** `bez = max(4, min(bezel, min(w,h) × 0.3))`。
注释里留了实测数据(标称 26px 时的边环占比):

| 面 | 尺寸 | 边环占高度 | |
|---|---|---|---|
| rail | 196×810 | 6% | 正确 |
| topbar | 1416×54 | 96% | **坏** |
| dock | 545×51 | 100% | **坏** |

边环是**锐利的、被位移过的背景**叠在磨砂之上;斜面宽过短边一半,整个面就变成一扇透明窗,底下每个字都会满对比度漏出来。
夹住之后,坞底下的字形能量从 4.28 降到 1.49(纯环境本底是 1.42)。

**能力门是双保险。** `@supports (backdrop-filter: url(#…))` 在 Firefox 与 Safari 里**判真然后什么都不画**,
所以 `@supports` 阶梯额外排除 `-moz-appearance` 与 `-webkit-named-image`,`PrismLens` 再在运行时探测一次并写 `html[data-lens="on|off"]`。
实测 Chromium:`data-lens="on"`,顶栏 / 侧栏 / 坞的 `backdrop-filter` 解析为 `url("#prism-lens-*")`。
透镜开启时 CSS tint 渐变按 `color-mix(… 72%, transparent)` 回收,避免双重磨砂。

### 3.8 试过并**否决**的参数(不要重试)

| 试过的 | 结果 | 现在是 |
|---|---|---|
| `--glass-blur: 20px` | 过度磨砂,把背景整个吃掉 | `12px` / 暗色 `13px` |
| 平铺 `rgba()` 玻璃体 | 读作卡片(旧基线首要病因) | 垂直 tint 渐变 |
| 1px 平色描边 | 没有玻璃边的两段受光感 | 遮罩梯度环 `::before` |
| 单个 32px 阴影 blob | 不浮 | 四段带负 spread |
| 轮廓光 `-9px` spread 配 `-8/-6px` 偏移 | 互相抵消,**什么都没画** | 已删,改内辉光 |
| 只用四盏宽 radial 光 | 平粉彩底,侧栏像卡片 | 加棱镜光带 |
| `--canvas-ambient` 单独当背景 | 频率太低,折射不可见 | `.ambient` 五层 + 噪点 |
| `--lens-rim-blur: 0` | 边环显示背后内容的针尖锐利压缩像 | `3` |
| `--lens-rim-blur ≥ 20` | 折射**完全消失**(条纹测试卡不再弯) | `3` 是唯一可用窗口 |
| 不夹斜面 | 短面变透明窗,字形满对比度漏出 | 夹到短边 30% |
| 用 `backdrop-filter` 模糊遮住坞底下的内容 | **无效**:面自身边缘 ~15px 内模糊核是单边的。用完整透镜链、裸 `feGaussianBlur(20)`、原生 `blur(20px)` 三种方式各验一次,结论一致 | 底部遮罩渐变(§4.4) |
| 参考原型的环境强度(约现值 2×) | 对 Operate 模式太吵 | 整体回撤 ~50% |
| 暗色玻璃"抬亮"背景 | 侧栏导航文字实测 **1.4:1** | 改熏黑 tint,实测 6.05:1 |
| 降级规则写成裸 `.glass` | 特异性 (0,1,0) 输给 `html[data-lens] .glass[data-pane]` (0,3,1),**静默失效**(实验室复现:`prefers-contrast: more` 下顶栏仍带着 `url(#prism-lens-topbar)`) | 每条降级都带 `.glass[data-pane]` |
| 坞净空写死 96px | 坞折行到两行后只剩 1px 余量 | 由 `--dock-h` 等推导 |

---

## 4. 三面预算与"内容必须从底下穿过"的布局法则

### 4.1 三面,不多不少

`GlassSurface` 是唯一的玻璃原语,全仓共 5 处调用:

| 面 | `layer` | `pane` | 何时存在 |
|---|---|---|---|
| `.topbar` | chrome | `topbar` | 常驻 |
| `.rail` | chrome | `rail` | 常驻 |
| `.dock` | chrome | `dock` | **仅当选中版本是 draft** |
| `.sheet-panel` | modal | — | 打开时 |
| `.unlock-card` | modal | — | 解锁页 |

`GlassSurface` 在 dev 下计数 chrome 面,超过 3 就 `console.error`。modal 层豁免。
实测:总览页 2 面;选中草稿后 3 面;再开验证 sheet → DOM 里 4 个 `.glass`,但 chrome 仍是 3。

### 4.2 让玻璃读作玻璃的那条法则

`.shell` 是 `position: fixed; inset: 0; overflow: hidden` 的视口盒,里面三个兄弟:

| | 定位 | z |
|---|---|---|
| `.topdeck` | `absolute` 顶部,`pointer-events: none`(子元素恢复) | 40 |
| `.rail` | `absolute` 左侧,浮着,**从不滚走** | 30 |
| `.canvas` | `absolute; inset: 0` —— **唯一的滚动容器**,满幅、实心 | 1 |
| `.dock` | `fixed`,底部居中胶囊 | 60 |
| `.sheet-backdrop` | `fixed`,portal 到 `<body>` | 100 |

`.canvas` 是满幅的(实测 `1440×900`,`inset: 0`),靠 `padding` 让内容避开玻璃,**而不是**靠布局网格给玻璃留行/列。
这就是关键差别:旧版把顶栏塞进网格的一行、侧栏塞进一列,中间隔 12px 沟 —— 玻璃背后什么都没有,于是拿掉 `backdrop-filter` 只改变 4–5/255 的像素。

### 4.3 一条 token 链,几何不会漂

`.shell` 上声明,派生量全部 `calc`:

| token | 值 | |
|---|---|---|
| `--shell-pad` / `--shell-gap` | `12px` / `12px` | |
| `--topbar-h` | `52px` | |
| `--conflict-h` / `--conflict-block` | `38px` / `0px` → 冲突时 `50px` | |
| `--rail-w` | `200px` | |
| `--rail-underlap` | `12px` | 内容面伸进侧栏内边的深度 |
| `--card-shift` | `gap + underlap = 24px` | 内容**面**左移的距离 |
| `--dock-h` / `--dock-inset` | `58px` / `18px` | |
| `--dock-clearance` | `0px` → 有坞时 `dock-h + inset + gap + 8` | 由坞的实际尺寸**推导** |
| `--canvas-top` | `12+52+0+12 = 76px` | 首行永不被遮挡的内容 |
| `--canvas-left` | `12+200+12 = 224px` | 首列永不被遮挡的**文字** |

实测(1440×900,亮/暗一致):顶栏 `(12,12) 1416×52`;侧栏 `(12,76) 200×392`;
首张卡片 `x = 200` —— 侧栏右边缘在 `212`,即卡片**恰好伸进侧栏 12px**。

### 4.4 下潜规则(underlap)

```css
.shell :is(.canvas, .canvas > section) > :not(section) {
  margin-left: calc(-1 * var(--card-shift));
  padding-left: var(--card-shift);
}
```

- **文字级块**把左移量原样当 padding 还回去 —— 字形不动;
- **卡片类面**(`.card` / `.stat-tile` / `.filter-bar` / `.card.tablewrap`)只还回 `card-shift − rail-underlap`,
  于是从**可见左边缘**量,它们的内边距和过去一模一样,而面本身伸到侧栏底下;
- **栅格容器**(`.overview-grid` / `.stat-row`)一点都不还 —— 它们的**第一列**像整宽卡片一样下潜,这就是画布左缘保持齐平、不呈锯齿的原因。

结果:侧栏折射的是**真实内容**,不是平背景;而**没有任何文字、单元格或控件坐在玻璃底下**。

### 4.5 滚动边缘(两侧)

`.canvas` 用两条 `mask-image` 渐变 `mask-composite: intersect`,锚在**边框盒(=视口)**上,内容穿过它们:

- `--edge-top`:`transparent` 到 `shell-pad+2px` → `rgba(0,0,0,.5)` 到 `chrome-bottom−14px` → 全黑于 `canvas-top`。内容在玻璃条里"透出鬼影"(玻璃有东西可折射),到条的上唇彻底消失。
- `--edge-bottom`:对称地在 `dock-inset−4px` … `dock-inset+dock-h+10px` 之间淡出;**仅当坞挂载时启用**。
- `--edge-top` 的 stacked 变体:顶部堆两条(桌面的 409 冲突条 / ≤720px 的侧栏横条)时,把鬼影窗**限制在顶栏之内**,避免内容从两条之间那 12px 没有玻璃的沟里钻出来。

底部这条**不是装饰**:它是唯一能遮住坞底下内容的机制(见 §3.8)。它零成本 —— 不增加任何 `backdrop-filter` 面。

`.canvas` 还带 `scroll-padding`(与 `padding` 同值):滚动口从视口顶边开始、即在玻璃底下,
不设置的话每一次浏览器驱动的滚动(Tab 聚焦、`scrollIntoView`、页内查找、`:target`)都会把目标停在顶栏底下。
另有 `scrollbar-gutter: stable`,避免经典滚动条平台在页面超过一屏时抖动列宽。

### 4.6 ≤720px

侧栏塌成顶栏下方的横条:`--rail-h: 44px`,`--rail-underlap: 0`,`--card-shift: 0`,`--canvas-left: --shell-pad`,
`--chrome-bottom` 随之长高一条。`.rail` 变 `flex-direction: row` 横向滚动,自绘滚动条隐藏
(15px 的经典滚动条会吃掉 44px 条的三分之一)。
`.rail-group` 与 `.rail a` 必须 `flex: none` —— 否则 flex 项**收缩**而不是溢出,每个标签会折成一列一字(44px 条里 5–6 行、被裁掉),横条也永远不会变成可滚动的。已在 700 / 560 / 420px 验证。

---

## 5. 材质语义与退火

材质编码的是**配置版本状态**,由 `GlassSurface` 的 `material` prop 写成 `data-material`。
顶栏与侧栏跟随当前选中版本的 `status`,坞恒为 `draft`。

| `data-material` | `--glass-blur` | `--glass-sat` | `--lens-frost` | `--lens-sat` |
|---|---|---|---|---|
| `draft` | `22px` | `120%` | `20` | `1.2` |
| `active` | `11px` | `195%` | `9` | `1.85` |
| `archived` | `16px` | `70%` | `13` | `0.8` |

语义:草稿是**更厚、更软**的一块玻璃;活动版本清澈而饱和;归档版本褪色。

**退火**:`.glass[data-material]` 上一条 `transition: backdrop-filter var(--dur-anneal) var(--ease)`,
`--dur-anneal: 600ms`,`--ease: cubic-bezier(0.32, 0.72, 0, 1)`。
发布成功后 `DraftDock` 重新 select 同一个版本,其 status 由 `draft` 翻到 `active`,
所有绑定材质的玻璃面因此走一次退火。这是**状态变化**,不是装饰动画 —— 这也是整套设计里唯一一条 transition。
`prefers-reduced-motion` 下 `--dur-anneal: 0ms`(实测)。

> ⚠️ 实测到的缺口,见 §10.2:透镜路径下材质当前**不生效**。

---

## 6. 排版 / 间距 / 圆角 / 动效

### 6.1 字体三角色

```
--font-text     -apple-system, "SF Pro Text", system-ui, "PingFang SC", "Hiragino Sans GB", sans-serif
--font-display  -apple-system, "SF Pro Display", system-ui, "PingFang SC", sans-serif
--font-mono     "SF Mono", ui-monospace, Menlo, Consolas, monospace
```

`.mono` 同时带 `font-variant-numeric: tabular-nums`。所有标识符、修订号、错误码、数量都走 `.mono`。

### 6.2 已建成的字号阶梯(全部实测/字面量)

| 用途 | 字号 / 字重 / 字距 | 颜色 |
|---|---|---|
| body | `14px / 1.5` | `--ink` |
| 页标题 `h2` | `21px / 700 / -0.02em`,display | `--ink` |
| 解锁标题 `h1` | `22px / -0.02em`,display | `--ink` |
| 区块标题 `h3` | `14px` | `--ink` |
| 统计数值 `.stat-value` | `24px / 700 / -0.02em` | `--ink` |
| 计数瓦片 `.count-value` | `22px / 700 / -0.02em` | `--ink` |
| 按钮 / `.small` | `13px / 600`(按钮) | — |
| 表头 `th` | `11px / 600 / +0.04em / uppercase` | `--ink-3`(重复下方数据,非正文) |
| 统计标签 `.stat-label` | `11px / 600 / +0.05em / uppercase` | `--ink-3`(同上) |
| 徽章 `.badge` | `12px / 600` | 状态色 |
| 芯片 `.chip` / `.idchip` | `12px / 500` / `12px` | `--ink-2` |
| 过滤栏 / 图例 / 版本选择器 | `12.5px` | `--ink-2` |
| 只读提示 / 计数标签 | `12px` | `--ink-2` |

墨水三层:`--ink` `#1d1d1f` / `#f5f5f7`,`--ink-2` `#6e6e73` / `#98989d`,`--ink-3` `#8e8e93` / `#6e6e76`。
**`--ink-3` 不是正文色**(亮 3.26:1 / 暗 3.56:1,低于 AA):句子用 `--ink-2`,`--ink-3`
只给图表网格线、重复下方数据的大写小标签和状态点。见 §10.10。
强调色 `--tint` `#0071e3` / `#0a84ff`;实心填充 `--tint-fill` `#0071e3` / `#0f6fd6`
(暗色必须分开:白字压 `#0a84ff` 只有 3.65:1)。
阶梯上**没有 11.5px** —— 运行时页曾自造这一档,已归到 12px。

### 6.3 间距

4pt 网格。壳体 `12px`;卡片内边距 `18px 20px`;单元格 `9px 14px`(首列 `20px`);
组件间常用 `4 / 6 / 8 / 10 / 12 / 14 / 16px`;栅格 gap `12–14px`。

### 6.4 圆角(同心)

`--r-outer: 20px`,`--pad: 8px`,`--r-inner: calc(20px - 8px) = 12px`(内半径 = 外半径 − 内边距)。

实际落地:

| 元素 | 半径 |
|---|---|
| 玻璃面 `.topbar` / `.rail` | `16px`(覆写 `--r-outer`) |
| `.card` | `16px` |
| 面内控件(导航项、输入框、`.chips-row`、`.count-tile`、`.capability-set`) | `--r-inner` = `12px` |
| 冲突条 | `12px` |
| 行内提示 `.action-error` / `.action-notice` | `10px` |
| 小 select / 小芯片 | `8px` |
| `.idchip` | `6px` |
| `.health-cell` / `.legend-dot` | `3px` |
| **胶囊**:所有 `button`、`.badge`、`.chip`、`.dock` | `100px` |

### 6.5 动效

- `--ease: cubic-bezier(0.32, 0.72, 0, 1)`,`--dur: 200ms`,`--dur-anneal: 600ms`;
- **整套设计层里只有一条 transition**:§5 的退火。`--dur` 目前没有任何规则引用(见 §10.6);
- `prefers-reduced-motion` 把 `--dur` 降到 `120ms`、`--dur-anneal` 降到 `0ms`;
- 焦点环:`:focus-visible { outline: 2px solid var(--tint); outline-offset: 2px; border-radius: 4px }`,全局一条。

---

## 7. 状态徽章词汇表与图表色板

### 7.1 徽章:闭集,色 + 点 + 文字,**从不只靠颜色**

`.badge` 带一个 `::before` 圆点(`currentColor`),文字始终存在。6 个色调:

| 色调 | 前景 | 底 / 边 |
|---|---|---|
| `good` | `--status-good` `#248a3d` / `#30d158` | `color-mix(… 12%)` / `30%` |
| `warn` | `--status-warn` `#b7791f` / `#ffd60a` | 同上 |
| `serious` | `--status-serious` `#c93400` / `#ff9f0a` | 同上 |
| `critical` | `--status-critical` `#d70015` / `#ff453a` | 同上 |
| `tint` | `--tint` | 同上 |
| `muted` | `--ink-2` | `--surface-2` / `--separator` |

`StatusBadge.tsx` 里的映射表就是词汇表本身(闭集枚举 → 色调):

```
draft·archived·disabled·missing → muted     active·available·fresh → good
cooldown·stale → warn                        circuit_open·quota_blocked·expired → serious
revoked·credential_forbidden → critical      recovery_required → tint
```

未知值一律落 `muted`,不猜。

### 7.2 图表色板(与状态色池**互不混用**)

| 槽位 | 亮色 | 暗色 |
|---|---|---|
| `--chart-1` | `#0066d6` | `#0a84ff` |
| `--chart-2` | `#b85f00` | `#cc7a00` |
| `--chart-3` | `#0079ab` | `#2196c9` |
| `--chart-4` | `#c41e77` | `#db4e92` |

规矩(已建成的用法):

- 顺序固定,**色随实体**:Token 构成永远是 输入 `chart-1` / 输出 `chart-2` / 推理 `chart-3` / 缓存读 `chart-4`;
- `SparkLine` 是隐没的:无轴、无标签,单条 2px `--chart-1`,`opacity: .75`;
- `MiniTimeline` 用 `chart-1` 画柱,失败量作为顶部的 `--status-critical` 帽子 + tooltip 文字冗余;
- `HealthStrip` 用状态色池(`ok/warn/bad` → `color-mix` 进 `--surface-2`),另有字形冗余;
- 状态色**只**用于徽章与状态标记,**永不**做分类序列色。
- 所有图形用 SVG **presentation 属性**(`fill="var(--chart-1)"`、`x`、`width`),不用 `style` 属性 —— CSP `style-src 'self'` 无 inline 豁免。

---

## 8. 无障碍降级与实测对比度

### 8.1 三条降级(每条都带 `.glass[data-pane]`)

| 条件 | 玻璃 | 滚动边缘 | 噪点 |
|---|---|---|---|
| `@supports not (backdrop-filter: blur(1px))` | `--surface` 实底 + `1px --separator` 边,伪元素 `display: none` | — | — |
| `prefers-reduced-transparency: reduce` | `color-mix(--surface 94%)`,无 backdrop,保留外阴影;`::before` 变纯 `--separator` 线,`::after` 隐藏 | 软渐变 → **硬切**在 `chrome-bottom` / 坞顶 | 隐藏 |
| `prefers-contrast: more` | `--surface` 实底,无 backdrop,`1px solid --ink-2` 边,**无阴影**,伪元素隐藏 | 硬切 | 隐藏 |
| `prefers-reduced-motion: reduce` | 不变(透明度与运动是两回事) | — | — |

实测 `prefers-contrast: more`:`backdrop-filter: none`,`background-image: none`,`border: 1px solid rgb(110,110,115)`,`box-shadow: none`。✔
实测 `prefers-reduced-motion`:`--dur: 120ms`,`--dur-anneal: 0ms`,透镜仍开启。✔

**特异性是契约的一部分。** 透镜规则是 `html[data-lens] .glass[data-pane]` = (0,3,1);
降级若写成裸 `.glass` = (0,1,0),会**静默输掉**。任何新降级都必须重复 `.glass[data-pane]`。

### 8.2 实测对比度(1440×900 @2x,画布已滚动 460px,即内容正从玻璃底下穿过 —— 最坏情况)

| 文字 | 亮色 | 暗色 |
|---|---|---|
| 侧栏非激活导航项(13px) | **4.52:1** | **6.05:1** |
| 顶栏 `.brand`(700) | 16.83:1 | 16.53:1 |
| 顶栏只读提示(12px) | 4.54:1 | 5.40:1 |
| 侧栏**激活**导航项(13px/600,`--ink` 坐在 `tint 16%` 芯片上) | **11.45:1** | **13.16:1** |
| 实心按钮白字(13px/600,`--tint-fill` 底) | 4.70:1 | 4.92:1 |
| 卡内说明 `.card-note`(12px,`--ink-2`) | 5.07:1 | 6.27:1 |

**地板是 4.5:1**(≤14px 正文按 WCAG AA 正常文本判定)。**全部通过。**

第四行曾是 3.25:1 / 3.91:1,是这个世界一度唯一的对比度缺口;修法与最初设想相反
(加浓芯片会让它更差),过程见 §10.4。第五、六行来自 impeccable 质量门(§10.10)。

`--ink-3`(亮 3.26:1 / 暗 3.56:1)**不在此表内,因为它不是正文颜色** —— 它只用于
图表网格线、重复下方数据的大写小标签和状态点。任何句子用它都是 bug,见 `tokens.css`。

> 测法:Playwright 截真实合成结果 → `data:` URL 解回 canvas → 取字形所在行内一块干净面片的**中位像素**做背景,
> 与元素 computed `color` 算 WCAG 比值。不靠肉眼,也不靠"理论上的 token 值"。

---

## 9. 如何扩展 —— 新页面要守的规矩

> 这一节是本文档存在的理由。

**玻璃**

1. **不要加第四面 chrome 玻璃。** 预算是 3(顶栏 / 侧栏 / 坞),dev 下 `GlassSurface` 会 `console.error`。
   需要一个新的浮动面 → 要么替换掉现有的一面,要么用 `layer="modal"`(sheet / popover / toast 层豁免)。
2. **永不玻璃套玻璃。** 玻璃面里的一切都是实心或透明,不能再有 `.glass`。
3. **内容永远实心。** 新的卡片、表格、图表一律 `.card` / `--surface`,放进 `.canvas`。玻璃不承载数字。
4. 新的 chrome 面若想要真折射:必须在 `PrismLens.tsx` 的 `PANES` 里登记一个 `id`,在 `glass.css` 的 `@supports` 阶梯里加一条 `[data-pane="…"]` 规则,并**在每条降级里也加上它**。

**布局**

5. **任何浮在画布上的固定元素,它的尺寸必须变成 `.shell` 上的 token**,然后由 `--canvas-top` / `--dock-clearance` 一类的派生量把净空推导出来 —— 不许写魔数(96px 的教训见 §3.8)。
   同时要给 `.canvas` 的 `padding` **和** `scroll-padding` 各加一份,再补一条遮罩渐变把内容在到达它之前淡掉。
6. **新页面的结构**:块直接放进 `.canvas`,或包一层 `<section>`;标签页再套一层
   `[role="tabpanel"]` 也可以 —— §4.4 的选择器**与层深无关**,`section` 与 `[role="tabpanel"]`
   都被当成只传递、不消费位移的布局层,所以位移既不会叠加也不会丢。
   用现成的 `.card` / `.stat-row` / `.overview-grid`,下潜规则会自动生效。
   **若发明新的布局包裹层**(不是卡片、只为分组存在),把它加进那两个 `:where()` 的名单,
   否则它的子级会整体右移一个 `--card-shift`,画布左边缘参差(教训见 §10.8)。
   **若发明新的栅格容器**,记得把它加进 `padding-left: 0` 那条列表,否则第一列会吃到双份左内边距。
7. 表格自己管水平内边距(`th/td:first-child`),包在 `.card.tablewrap` 里。

**颜色与材质**

8. 颜色只能从 token 里取。组件里**不许出现裸 hex**。
   状态色池(`--status-*`)只给徽章与状态标记;分类色板(`--chart-1..4`)只给图表序列。两者不互换。
   **墨阶按用途选,不按"想多淡"选**:句子只能用 `--ink` 或 `--ink-2`;`--ink-3`
   (亮 3.26:1 / 暗 3.56:1)只给图表网格线、重复下方数据的大写小标签、状态点。
   实心 `--tint-ink` 文字的底必须是 `--tint-fill`,不是 `--tint`(暗色下后者只有 3.65:1)。
   要更"安静"的层级,用字号、位置和留白,不要用不达标的颜色(教训见 §10.10)。
9. 新的材质状态要同时给**两条路径**的四个值,否则在其中一条上是空操作:
   `--mat-<态>-blur` / `-sat`(分层兜底)与 `--mat-<态>-frost` / `-lsat`(SVG 透镜),
   都写在 `tokens.css`,再由 `glass.css` 的 `[data-material="<态>"]` `var()` 引用。
   **不要在 `glass.css` 里写字面量** —— 那正是两条路径分叉过一次的原因(§10.3)。
   透镜路径的退火由 `PrismLens` 的 JS 补间负责,新态自动继承,无需额外接线。

**可访问性**

10. **玻璃上的文字必须实测,不许目测。** 地板 4.5:1(≤14px),取"内容正从底下穿过"的最坏帧。
    句子的最坏底色是 `--surface-2`(不是 `--surface`),按它验;若一段文字的父级一路到
    `<body>` 都没有实心背景,它就是坐在环境层上,必先归位再谈颜色(§10.10)。
11. 每个新表面都要在 **亮 / 暗 × 正常 / `reduced-transparency` / `contrast: more` / `reduced-motion`** 下看过。
    降级选择器必须带 `.glass[data-pane]`(§8.1)。
12. 状态永远"色 + 形/文字"双编码,不许只靠颜色。

**代码形态(机械检查,`npm run check`)**

13. 没有 `style={{}}`。静态样式进 `app.css` 的类;动态几何走 SVG presentation 属性。生产 CSP 是 `style-src 'self'`。
14. 没有任何浏览器存储;`src/generated` 之外没有裸 `fetch(`。
15. 动效只用于**状态变化**,不做装饰。要加 transition 前先问:它编码了什么状态?
    交互反馈用 `--dur`(hover / press,**只过渡颜色**);配置版本换态用 `--dur-anneal`。
    两者都在 `reduced-motion` 下被缩短或归零 —— 写死毫秒数会绕过这层。
16. 中文文案,克制、无废话,字符串进 `src/i18n/messages.ts`。
    空态必须区分四种:`真空`(暂无数据)/ `过滤后空`(没有符合过滤条件的结果)/ `投影不可用`(此部署未启用该运行时投影)/ `管道未接线`(`[data-kind="unwired"]`,虚线边框)。
17. 值无关纪律:后端只给闭集枚举与标识符,永不给请求体。UI 不得暗示能看到内容。
18. 提交前跑 `npm run check`。

---

## 10. 已知张力

### 10.1 环境的饱和度 vs Operate 模式的克制

这是这个世界与"运维面板应该安静"之间最直接的冲突。
原型里的环境层是为**最大可见折射**调的,搬进产品后明显太吵。

**已决定**:环境强度与棱镜光带整体回撤 ~50%(§2.3 就是回撤后的值),但**保留光带**——
它是唯一提供小尺度色相变化的层,没有它侧栏立刻退回"卡片"(§2.2 有实测)。
代价是画布之外那圈边缘确实是彩色的、在暗色下尤其可见(见 `final-dark.png` 左下角的青紫楔形)。
守住克制的办法不是把环境调灰,而是**内容画布永远实心**:数据从不坐在彩色上。

### 10.2 材质语义在透镜路径下当前不生效(实测)—— 已修复

**原症状**:`PrismLens` 只在挂载与 resize 时读 `--lens-frost` / `--lens-sat` 写进 SVG 滤镜。
切换 draft ↔ active 会改 CSS 变量,但**不会**重跑 `updatePane`:

| 状态 | `.topbar` 的 `--lens-frost` | `#prism-lens-topbar` 实际 `stdDeviation` |
|---|---|---|
| 选中 draft | `20` | `9` ← 陈旧 |
| 强制 resize 后 | `20` | `20` ✔ |

因为 `data-lens="on"` 时 `backdrop-filter` 是 `url(#…)`,CSS 的 `--glass-blur/-sat` 被完全绕过。
同理,`.dock` 的滤镜在挂载时 `feImage href=""`、`data-key=null`(实测),要到下一次 resize 才建出位移图 —— 坞渲染成一块纯磨砂胶囊,没有折射边环;而 `href=""` 还在控制台刷出三条 React 报错。

**根因有两处,不是一处:**

1. 三个面全是 `position: fixed`,**谁都不会改变 body 的盒子**,所以挂在 `document.body` 上的
   `ResizeObserver` 对"坞挂载"这件事完全无感 —— 坞的位移图不是"晚一点建",是**永远不建**。
2. SVG 滤镜基元是**属性**,不是可动画的 CSS 属性。`transition: backdrop-filter` 在
   `url(#…)` 下是彻底的 no-op:退火不是"读不出来",是**一帧之内硬切**。

**修法(已落地,实测复核):**

- `.shell` 上加一个 `MutationObserver`(`attributeFilter: ["data-material","class"]` + `childList`),
  按帧合并成一次 pane 扫描;`ResizeObserver` 除 body 外**逐面 observe**。
- `--lens-frost` / `--lens-sat` 改为**每次 `updatePane` 都重读**,不再只在重建位移图时读。
- 退火改成 JS 补间:同一个 `--dur-anneal` 与 `--ease`(`cubic-bezier(.32,.72,0,1)` 用二分求值),
  每个滤镜 id 一个在跑的 tween,中途再次换态就替换而不是竞争;`--dur-anneal: 0ms`
  (reduced-motion)自动退化成直接赋值。
- `feImage` 去掉 `href=""` 初值(改为不设该属性),三条 React 报错消失。

实测(Chromium,`data-lens=on`):

| | `feImage href` | map 宽 | rail `stdDeviation` | rail `saturate` |
|---|---|---|---|---|
| active | `data:image/png…` | 200 | `9.00` | `1.850` |
| 选中 draft | `data:image/png…` | 200 | `20.00` | `1.200` |
| dock(draft 时挂载) | `data:image/png…` | **372** ← 原为 `""`/10 | `20.00` | `1.200` |

退火中途采样(draft → active,600ms):`18.35 → 10.06 → 9.00`,即真在补间。
回归由 `e2e/glass.spec.ts` 两条用例锁住(每个已挂载面都必须有真位移图;换态必须移动透镜且中值严格落在两端点之间)。

### 10.3 契约 §8.2 / §8.4 的数值已被实测推翻

契约写 `blur 20 / sat 180%`,以及材质 `draft 28/120`、`active 14/190`、`archived 20/80`。
建成的世界是 `blur 12`(暗色 13),材质 `22/120`、`11/195`、`16/70`。
原因见 §3.8 第一行:20px 过度磨砂。**以本文为准。**

补记:这三组材质数值现已收进 `tokens.css` 的 `--mat-*`(见 §10.6),
`glass.css` 只 `var()` 引用 —— 两条渲染路径(分层兜底 / SVG 透镜)从同一处取数,
不可能再出现"一条路径动了另一条没动"。

### 10.4 激活导航项的对比度低于 AA —— 已修复(修法与最初设想相反)

**原症状**:`tint` 文字压在 `tint 16%` 芯片上,实测 3.25:1(亮)/ 3.91:1(暗),低于 4.5:1。

最初记录的两个修法方向是"把芯片提到 ~22%"或"激活文字换 `--ink`"。**第一个方向实测是错的**:
芯片本身就是 `--tint` 调出来的,加浓等于把背景朝文字颜色推,亮度差必然缩小。实测 22% 时
亮色 3.20 → **2.93**、暗色 3.92 → **3.65**,比原来更差。0%~100% 全扫一遍:

| 芯片浓度 | 0% | 16% | 22% | 45% | 80% | 100% |
|---|---|---|---|---|---|---|
| 亮·tint 文字 | 3.63 | 2.98 | 2.76 | 2.05 | 1.29 | 1.00 |
| 亮·ink 文字 | 13.01 | **10.69** | 9.89 | 7.33 | 4.62 | 3.58 |
| 亮·白文字 | 1.29 | 1.58 | 1.70 | 2.30 | 3.65 | **4.70** |

蓝字压蓝底**没有任何浓度**能到 4.5:1 —— 连完全去掉芯片(0%)也只有 3.63。
这个方向不是"效果不够好",是算术上不可能。

**已落地**:芯片保持 16%,激活文字换成 `--ink`。实测 **11.45:1(亮)/ 13.16:1(暗)**
—— 取最坏帧(画布已滚动 460px,真实内容正从导轨底下穿过,玻璃有内容可折射)。
蓝色没有消失,它整体移进了芯片底;标记激活的仍是那抹蓝,只是不再由文字承担对比度。

### 10.5 注释与代码的一处措辞不符

`tokens.css` 暗色段注释写"暗面必须把背景**抬起来**",而实际 tint 是 `rgba(12,12,18,…)`,是**熏黑**。
注释描述的是调参前的方向;调参后暗色改成熏黑,正是把导航文字从 1.4:1 救到 6.05:1 的那次改动(见 commit 5df23a7 尾段)。
以代码为准。

### 10.6 已建成但未接线的 token —— 已接线

原记录:6 个 `--mat-*` 与 `--dur` 声明了但没有任何规则引用。两者都已接上,顺带修掉了各自背后的实质问题。

- **`--mat-*`**:原先真正生效的是 `glass.css` 里的字面量,`tokens.css` 那份是**另一套过时数值**
  (`draft 28/120`、`active 14/190`、`archived 20/80` —— 环境层还不存在时调的)。
  现在 `tokens.css` 收下实测值并**扩成 4 组 × 3 态**:`--mat-*-blur/-sat` 给分层兜底,
  `--mat-*-frost/-lsat` 给 SVG 透镜;`glass.css` 只 `var()` 引用。单一数值源,两条路径不可能再分叉。
- **`--dur: 200ms`**:原先无人引用 —— 也就是说所有 hover 态都是**硬切**,而
  `prefers-reduced-motion` 段里那句 `--dur: 120ms` 缩短的是一个不存在的动效。
  现已接到导航项、chip、count-tile 的 hover 上(**只过渡颜色,不动几何**),
  共 5 处引用,reduced-motion 覆盖这才有了实际作用对象。
- `--canvas-ambient` 仍只被 `.unlock-scene` 和 `.shell` 的兜底背景用到 —— `.shell` 那份被
  `.ambient` 整个盖住,实际不可见。保留是因为它是契约里的名字,但**新代码不要引用**。

### 10.7 三面预算只在 dev 下强制

`GlassSurface` 的计数器是 `import.meta.env.DEV` 下的 `console.error`,`scripts/check.mjs` 不检查它。
预算靠人守。加面之前先数一遍。

模态层同理:`Sheet` 也有一个 dev 下的 scrim 计数器(上限 1 —— 全应用只有这一处
全视口 `backdrop-filter`),同样不进 `check.mjs`。

### 10.8 内容左边缘曾因多一层包裹而参差(已修复)

`app.css` 的下垫机制原先只匹配两层(`.canvas > *` 与 `.canvas > section > *`)。
用量分析页在 `<section>` 里还套了一层 `[role="tabpanel"]`,于是它的卡片**整体右移 24px**:
实测 `x=224`,而同页 filter-bar 与运行时/监控页的所有卡片都在 `x=200` —— 画布左边缘参差,
且该页的卡片根本没有下垫到导轨底下(玻璃少了它该折射的那部分内容)。

修法是把匹配做成**与层深无关**(`section` 与 `[role="tabpanel"]` 都视为只传递、不消费位移的布局层),
而不是给用量分析页开特例。附带踩到一个特异性陷阱:加了 `:is()` 之后位移规则反而压过了
下面那批"归还 padding"的规则,直接子级卡片的内 padding 从 32px 掉到 24px;
把结构部分放进 `:where()`(权重归零)才回到原值。

实测三页(1440×900):全部满宽卡片 `x=200`,内 padding 与改动前逐项一致。
`e2e/glass.spec.ts` 第三条用例锁住"跨页只有一个左边缘"。

### 10.9 `check:full` 曾会因陈旧 dist 误报不可复现

`check.mjs` 为了让日常 `check` 走快路,`dist` 存在时就复用它。但 `--double-build` 下这条
捷径是错的:第一份哈希取自**上一次构建**,第二份取自当前树,于是任何陈旧 `dist` 都会报
一次并不存在的 C10 失败(本次就撞上了)。现已改为 `--double-build` 时先强制清一次 `dist`,
两半都出自当前树;顺带让 C3/C4 的产物断言也不再有机会检查陈旧构建。
已用"故意塞一个陈旧文件进 dist"验证不再误报。

### 10.10 impeccable 质量门:一次跑遍十页的结果

用 impeccable 4.0.2 的浏览器规则集(8208 行)扫全部十页 × 亮/暗 × 1440/390。
CLI 静态路径对故意写坏的探针文件也返回零,规则实际全在浏览器引擎里,而它硬依赖
puppeteer;本仓库装的是 Playwright,所以改用 `page.evaluate` 注入(隔离世界,不受
`script-src 'self'` 限制)跑同一份规则集。

**修掉的真缺陷:**

| 缺陷 | 实测 | 处理 |
|---|---|---|
| `.card-note` 坐在环境层上 | 亮 **2.2:1** | 挪进它所解释的那张卡内(内容永不坐环境层,§9 规矩 3) |
| `--ink-3` 用于正文 | 亮 3.26 / 暗 3.56 | 见下 —— 系统性问题,13 处引用 |
| 实心蓝按钮白字(暗色) | **3.65:1** | 新增 `--tint-fill`(暗 `#0f6fd6` → 4.92:1) |
| `.rt-op` / 矩阵列头 11.5px | 不在字号阶梯上 | 归到阶梯的 12px |

`--ink-3` 那条是**系统性**的,检测器只报了当前视口里可见的一条。它在两个主题下分别是
3.26:1 / 3.56:1,却被 13 处引用,其中若干是用户必须读的句子。
**把 token 本身提亮不是解**:任何在 `--surface-2` 上达标的值都会落进 `--ink-2` 的
0.1 以内,三级墨阶塌成两级。所以按**用途**拆:`--ink-3` 明确降级为非正文用途
(图表网格线/轴刻度、重复下方数据的 11px 大写列标签、状态点),凡是句子改用 `--ink-2`。
`tokens.css` 里写了这条约束,`.muted-3` 也一并改为 `--ink-2`(它 6 处调用中有 4 处是句子)。

**判定为假阳性(不改):**

- `line-length ~168 字符/行` —— `max-width: 78ch` 生效(614px),实测每行约 56 个拉丁字符宽;
  规则按拉丁字宽换算,而这里是中文,单字约两倍宽。实际约 28 字/行,正好在中文舒适区。
- `em-dash-overuse` —— 中文破折号是正规标点,不是英文写作里的滥用。
- `clipped-overflow-container` —— `.shell` 确实 `overflow: hidden`,但实测**没有任何**
  定位子元素越界(`position: fixed/absolute` 且超出 `.shell` 盒子的数量为 0)。
- `health-cell` 白色 "!" 压 `--status-critical`(暗 3.4:1) —— 10px 的图标性冗余标记,
  信息由形状与 `title` 承载,容器是 `role="img"`;它不是需要阅读的文字。
- `radial-spotlight-glow`(10 页) —— 这正是本设计系统的环境层。玻璃背后没有东西时不可能
  看起来像玻璃(见 §2),这条是刻意的,不是"AI 反射式装饰"。

**判定为刻意取舍(不改):**

`flat-type-hierarchy`:七页字号跨度 21/11 = 1.9:1。规则只报了没有 `.stat-value`(24px)
的那七页,即它测的是"页内最大∶最小",不是设计系统的完整阶梯。运维台密度优先,
不需要营销页那种字号跨度;层级由字重、位置与颜色承担(§6.2)。**以后不必反复重提这一条。**

