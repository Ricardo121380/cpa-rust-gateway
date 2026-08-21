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
- **序列上限就是 4**(`COMPARE_LIMIT`)。第五条序列没有可用色阶 —— 要么复用色相、
  要么去借状态色池,两者都会让图说谎。需要看更多实体时换排行表,不要加第五条线。
- **多序列必须双通道编码**:除色相外各带一种虚线样式(实线 / `7 4` / `2 3` / `9 3 2 3`)。
  `@media (forced-colors: active)` 下色相全部塌成 `CanvasText`,虚线与线宽是仅存的区分通道;
  图例色块本身画一段带同样虚线的线,所以不悬停也能对上(§12.2)。
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
16. 文案先写 `src/i18n/zh.ts`(唯一真源),再在 `src/i18n/en.ts` 补上对应键 ——
    `Pack` 由 zh 推导,漏译是**类型错误**而不是静默回落中文。组件用
    `useMessages()`(响应语言切换);旧的静态 `messages` 只剩 `AppShell` 一处类型引用,
    **新代码不要再用**,否则该处文案切不了语言(教训见 §11)。
    **不翻译的东西**:错误码、scope、stage、重试决策、endpoint/credential/model 标识符 ——
    它们是契约值不是文案,翻了日志就搜不到了。
    空态必须区分四种:`真空`(暂无数据)/ `过滤后空`(没有符合过滤条件的结果)/ `投影不可用`(此部署未启用该运行时投影)/ `管道未接线`(`[data-kind="unwired"]`,虚线边框)。
17. 值无关纪律:后端只给闭集枚举与标识符,永不给请求体。UI 不得暗示能看到内容。
18. **视图状态入 URL,但先查参数名有没有被占。** 过滤器、时间窗、标签页、展开态、
    缩放范围、选中桶都进 query,分享链接必须能复现同一屏。共享的时间契约
    (`utils/timerange`)已经占了 `range` / `from` / `to` / `bucket` —— 我一度用
    `bucket` 存"选中的桶",那会让选中一个桶悄悄改变整张图的粒度(现为 `at`,见 §12.5)。
    **能被分享的东西不能靠会话状态**:刷新清空内存会话(规矩 14),所以状态必须完全
    可从 URL 重建,不是"刷新后还在"。
19. 提交前跑 `npm run check`。

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
(设置页当时还不存在;它建成后按同一套流程复扫过,结果见 §11。)
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


---

## 11. 设置页与双语:两条"只存于内存"的偏好

设置页是全站唯一会改全局状态的页面,也是最容易做假的一页 —— 网关**没有** settings
端点,面板又禁止任何浏览器存储,所以它能诚实承载的只有会话内的东西。

**它不做什么**:不假装持久化。导语第一句就说明每一项刷新后回到默认值,并说明这是刻意的。
四个分区里有两个是**只读**的:渲染能力与系统无障碍偏好是探测来的,不是选出来的,
把它们做成开关会暗示面板能覆盖操作系统。

| 分区 | 类型 | 载体 |
|---|---|---|
| 外观 | 可选(3 态) | `themeStore` → `html[data-theme]`,即 tokens.css 三层链的第三层;`system` 表示**不写属性**,把决定权交回 `prefers-color-scheme` |
| 语言 | 可选(2 态) | `useLangStore` → `useMessages()` |
| 会话 | 只读 + 一个动作 | 密钥**从不渲染**,只给 `…后四位 · 长度`(前缀由契约固定,不含信息);"锁定并清除密钥"清空内存并退回解锁页 |
| 渲染能力 / 系统偏好 | 纯只读 | `html[data-lens]` 与四条 media query,用 `useMediaQuery` 订阅(用户可能开着面板去改系统设置,一次性读会读旧) |
| 构建 | 纯只读 | `import.meta.env`,其中 fixture 模式必须显式写明"数据是本地伪造的" |

### 11.1 i18n 的形状

zh 是唯一真源,`Pack` 从它推导并把叶子放宽为 `string`(不放宽的话 `as const` 会把每个值
钉成自己的字面量类型,任何译文都无法满足)。漏译因此是**类型错误**:删掉 en 里一个键,
`tsc` 直接指出 "Property 'contract' is missing"。

**不翻译**:错误码、scope、stage、重试决策、各类标识符。它们是契约值,翻译会让一条日志
再也搜不到。

### 11.2 接线时踩到的真问题

侧栏导航标签原先在**模块作用域**算一次(`NAV_GROUPS` 里直接写 `messages.nav.overview`),
所以切语言后整条导轨永远停在中文 —— 页面自己是对的,壳不对。改成只存 `key`、
渲染时查表;顺带让 E2E 的导轨定位器不再依赖译文。

同类问题在顶栏("当前版本只读"、冲突条,以及一处硬编码的"知道了")和解锁页也存在。
现在 40 处调用点全部走 `useMessages()`,`messages` 静态导出只剩 `AppShell` 一处类型引用。

`e2e/settings.spec.ts` 锁住三件事:切语言必须传到**导轨与顶栏**(不只是当前页)、
`system` 必须不写 `data-theme`、以及密钥的完整值不得出现在 DOM 里的任何形态中。

### 11.3 建成后复扫

按 §10.10 同一套流程(11 页 × 亮/暗 × 1440/390)复扫,新页面带出**一条**真缺陷:
`.settings-lead` 导语直接坐在环境层上,实测 3.5:1 —— 与 `.card-note` 一模一样的错,
而且是在我刚把这条规矩写进 §9 之后犯的。已挪进实心卡内。

顺带清掉一处重复:`.chip-on` / `.chip-off` 原先在 `.preset-chips`、`.usage-chips`
两处各自定义一遍,设置页要用又会漏 —— 现已提为 app.css 里的通用类,各特征只管尺寸。

复扫后 low-contrast 在亮/暗/窄屏三档全部归零,余下的都是 §10.10 已判定的假阳性与刻意取舍。

---

## 12. FE-4:第六子页、实体对比、JSONL 导出

### 12.1 补齐的差口

契约 §7.2 要求用量分析**六**子页,建成的只有五个 —— 缺 Client Key。补上后
`USAGE_TABS` 与契约一一对应,`includeForTab` 里三个排行页各自请求自己的维度
(`public_model` / `client_key` / `credential`)。

顺带修掉一个 fixture 里的谎:它的 `ranks` 无视 `by`,任何维度都返回模型列表。
如果不修,新页面看上去是"接线好的",实际展示的是别的维度的数据 —— 这比空态更坏。

### 12.2 实体对比:为什么是一页一图 N 次查询

契约的 `timeline` **不分段**,没有"按实体拆分"的投影。要 N 条序列就只能发 N 次
带单值过滤的查询。我没有自造一个分段响应形状 —— 那会是后端从未承诺过的投影。

代价是 N 次往返,所以**默认收起**,展开状态入 URL(`compare=1`),分享链接能复现。
排行表本身已经回答了"谁最大";展开对比是用户明确想看"随时间的形状"。

**上限 4,不是任意 N**:分类色板只有四个验证过的色阶,第五条要么复用色相、
要么去借状态色池 —— 两者都会让图说谎(§7.2)。

**固定色序**:排名顺序就是颜色顺序。第一名恒为 `--chart-1`,只要排名不变,
刷新之间色相不会重排。掉出前 N 的键返回 `-1`,由调用方显式处理,而不是悄悄改色。

**双通道识别**:四条线除色相外各有虚线样式(实线 / 7-4 / 2-3 / 9-3-2-3)。
这不是装饰,是 forced-colors、色觉障碍与灰度打印下唯一还在的通道 ——
`@media (forced-colors: active)` 里色相全部塌成 `CanvasText`,虚线与线宽是仅存的区分。
图例的色块本身就是一段带同样虚线的线,所以不悬停也能对上。

**仍然单轴**:四条序列是**同一度量的不同实体**,这正是它们可比的前提。
换第二根轴,两条线相交就不再有任何含义。

### 12.3 导出:客户端做,导入不做

- **导出**在客户端完成。行已经在 `useInfiniteQuery` 的页里,再走一个服务端流式导出
  等于给同一个窗口造第二个真相源;而且流式端点属于 G3 附属,尚未交付。
  用户拿到的文件就是表格刚显示的东西。
- **导入不做**。分块续传导入必须有后端端点。给一个禁用按钮或一个静默丢文件的选择器
  都比明说更坏 —— 页面直接写明"需要 G3 附属端点(尚未交付)"。

文件形状:第一行是**自描述头**(format / 窗口 / 过滤条件 / 行数 / `partial`),
其后一行一条记录。头是必需的:一份过滤后的导出如果不标明,和完整导出无法区分;
只加载了两页就导出会标 `partial`,文件名也带 `-partial`。

脱敏不是可选项(契约 §189"导出与展示同一脱敏规则"):导出字段集**就是表格渲染的字段集**,
闭集枚举、标识符、计数、时间戳。没有请求体 —— 契约里本来就没有,文件也不能暗示有。
单测里有一条专门断言 `body` / `prompt` / `messages` / `content` 这类键不存在。

`URL.createObjectURL` + `a[download]`,不是 `data:` URL:前者不受 `connect-src 'self'`
约束(不涉及 fetch),也省掉整个文件的一次字符串拷贝。URL 在下一个 tick 撤销,
不撤销会把 blob 钉在内存里直到会话结束。**已在真实 CSP 下验证**:26 行(1 头 + 25 行)、
每行独立可解析、标记 partial、无任何请求体字段。

### 12.4 复扫

按 §10.10 的流程复扫,导出栏带出一条 `cramped-padding`:`.card.tablewrap` 故意没有横向
内边距(由表格的 `th/td:first-child` 承担),而这行 header 不是表格,于是贴到了卡片右边框上。
给 header 自己补 `padding-right`,不动 tablewrap 的既有约定。修后计数回到 26,与新页面加入前持平。

### 12.5 趋势页范围条与选中桶

契约 §7.2 要求趋势页有"12 桶以上的内置 dataZoom"与"选中桶虚线标记"。两项都补齐,
且都**只重取已加载的窗口,不重新请求** —— 范围是从同一份 buckets 里切出来的,
不是另一个分辨率的数据。E2E 用监听 `/admin/analytics` 请求数为 0 来锁这一点。

**范围条不是通用 dataZoom**。它只在一根轴上选一段连续桶,这也是分析契约唯一能表达的缩放。
它把**整条序列**画成低对比火花线、选区外压暗,所以控件回答的是"我正在从什么里面切",
而一对光秃秃的数字输入框回答不了。

两个滑块是真的 `<input type="range">`,不是自绘 SVG 手柄:后者是纯指针可达的,
前者免费获得键盘操作、屏幕阅读器标签与触屏可用。只替换外观,不替换语义。
两个手柄不可交叉 —— 起点被推过终点时会把终点一起带走,而不是产生反向窗口。

**选中桶按开始时间存,不按索引存**(URL 参数 `at=`)。索引在缩放或粒度变化后指向不同的桶,
分享出去的链接会落在错的地方。参数名不能用 `bucket` —— 那是共享时间契约里的粒度参数,
复用会让"选中一个桶"悄悄改变整张图的分辨率。

标记是**虚线 + 空心环**,与临时的十字准星靠**形状**区分,不是只靠颜色;
它同时在配对数据表里高亮对应行,所以标记不是图表独有的。图表可键盘操作(方向键走光标、
Enter 提交),所以点击可达的事键盘也必须可达。

### 12.6 排行表展开面板

契约 §7.2 的"展开对比面板"。展开行是一个 `colSpan` 跨满列的 `<tr>`,不是嵌套表格 ——
浏览器保持同一套列栅格,面板始终对齐在它所属的那一行下面。

**懒挂载**:面板自己发查询,所以收起的表格一次请求都不发;同时只允许展开一行。

踩到一个视觉陷阱:展开行的底色就是 `--surface-2`,而占比条的轨道也是 `--surface-2`,
于是轨道消失、条看起来比邻行短。**实测宽度完全一致**(26% → 31.2px,20.5% → 24.6px),
消失的只是轨道。修法是展开行里把轨道从所在表面再推开一档,而不是去动条本身。

另外两处一并修掉:范围条的滑块原先浮在轨道下方(改为压在轨道底部,看起来是"抓住"窗口边缘);
`.bucket-facts` 的 `auto-fit` 在窄屏会把一对 dt/dd 折到下一行、标签与上一行的值错位,
改为固定列数 + 显式 `grid-row`。

一处**不是缺陷**的现象记录在此,避免以后重复排查:选中桶后刷新页面会退回解锁页,
选中状态"丢失"。这是 C6(禁止任何浏览器存储)的必然结果 —— 刷新清空内存会话。
我最初的 E2E 断言了"刷新后仍在",那是在拿测试对抗安全模型;
改为断言 URL 契约本身:重新导航到同一 URL 必须复现同一个选中桶。

---

## 13. G2 部分落地:网关实时计数与观测管道健康

后端在 2026-08-09 的契约里加了 5 个算子(纯增量,65→70,零删除),其中
`GET /admin/observability/metrics` 是本节的全部来源。前端此前把整个观测面
当成一块"等 G2+G3"的空白,这是错的:G2 的一半已经在了。

### 13.1 这个平面能说什么、不能说什么

端点返回 **Prometheus 文本曝露格式**(`text/plain; version=0.0.4`),
7 个指标族,全部是**自网关进程启动的累计计数器**,重启归零。
渲染器的注释写得很明确:"never adds request-scoped or target-scoped label"。

所以:

| 能 | 不能 |
|---|---|
| 上游尝试成败(`attempts_total{outcome}`) | 任何时间桶 —— 没有时间维度 |
| Token 按类别(`usage_tokens_total{kind}`) | 按模型 / 按 Key / 按凭据切分 |
| 事件按类别(`events_total{kind}`) | 延迟分位 —— 指标里根本没有延迟 |
| 队列与持久化的丢弃计数 | 单条请求 —— 没有请求标识 |

**不要从这些计数器推导 G3 的任何东西。** 缺的不是聚合逻辑,是标签维度本身;
凑出来的"今日"或"按模型"会是编造的。总览页因此是两个**分别标注**的平面:
「网关实时计数 · 自进程启动累计」与「今日分析 · 按时间窗」。

这条标注不是排版偏好。接线后第一版两个统计行都没有标题,页面上先后出现
`成功率 94.98%` 和 `成功率 98.72%` —— 同一个词两个数字,读起来像 bug。
两个平面各自带标题与徽章之后才消歧。

### 13.2 严重度:丢弃不等于丢弃

`queue_admission_total{outcome="diagnostic_dropped"}` 与
`durable_events_total{outcome="required_quarantined"}` 都是"事件没留下",
但意义相反:队列把 Required 和 Diagnostic 分开,**就是为了在压力下先丢诊断**。
诊断丢弃是背压设计在工作,Required 丢失是持久化日志真的丢了数据。

`metrics.ts` 因此给每个丢弃源带 `severity`,UI 分开呈现:Required 为零时是
`badge-good`「必需事件无丢失」,非零是 `badge-critical` 加明细表;诊断丢弃永远
只是一句灰色附注,并明写"背压设计,非故障"。把两者用同一个音量喊出来,
只会训练运维忽略真正要紧的那个。

**这是 UI 里唯一的数据丢失告警面** —— G3 落地后也不会替代它,分析端点报的是
请求,报不了"事件日志自己丢了多少"。

### 13.3 累计值没有用,增量才有用

`gateway_observability_attempts_total 1254` 这种进程生命期数字对运维几乎无信息。
有信息的是"我打开这页之后涨了多少"。所以留一个 `baseline` ref。

坑在这里:baseline 一旦在首次抓取时设好,`growthSince` 立刻返回 `{attempts: 0}`,
页面第一帧就显示「本页 +0」。那是**对一个根本不存在的观测窗口下结论** ——
此刻还没有第二次抓取。修法是把 baseline 连同它那次抓取的 `dataUpdatedAt` 一起存,
只有 `dataUpdatedAt` 前进过才谈增量。这样「+0」出现时是真信息(轮询了一轮,网关空闲),
而不是占位符。E2E 两头都锁:首帧不得出现「本页 +」,时钟推进 16s 后必须出现。

`growthSince` 另外在计数器变小时返回 undefined —— 那意味着网关重启,基线已经没有意义。

### 13.4 传输:契约里第一个非 JSON 响应

生成客户端的 `request()` 返回原始 `Response`,但 `call<T>()` 一律走 `.json()`。
Prometheus 是 text/plain,于是把 `call()` 的请求路径抽成 `send()`,
加一个 `callText()` 走 `.text()`。fetch 仍然只在 `src/generated` 内发起,C5 不破。

`src/api/prometheus.ts` 是纯解析器(`parsePrometheus` + `pick`),
非有限值的行直接丢弃 —— NaN 漏到 StatTile 上会渲染成 "NaN"。

### 13.5 Token 卡为什么会让位

`TokenSummary` 的字段与 `usage_tokens_total` 的 `kind` 标签**完全对上**,
`TokenMixBar` 零改动即可消费累计值。但 G3 在场时它的「今日」版本严格更有用,
两张同形状的 Token 卡并列只是噪音 —— 所以累计版只在 `analyticsAvailable()`
为假时渲染。这也意味着它在 fixture dev 下没有 E2E 路径,由单测覆盖形状。

腾出的位置给「事件构成」:这是已经解析、原本被丢掉的数据,
回答"管道到底有没有收到用量事件",且让栅格不再是一张孤零零的整宽空卡。

### 13.6 一个**不是** bug 的东西 —— 别再"修"一次

上一轮留了一条待办:"轮询在后台标签页不暂停,只有 RuntimePage 做对了,
应把它的 `visibilitychange` 模式推广到另外 4 个页面"。**这条是错的。**

TanStack Query v5.101.4 默认就不在隐藏标签页发请求。
`queryObserver.js:215` 的定时回调是:

```js
if (this.options.refetchIntervalInBackground || focusManager.isFocused()) { … }
```

`refetchIntervalInBackground` 全仓没有任何赋值(falsy),而
`focusManager.isFocused()` 就是 `document.visibilityState !== "hidden"`。
实测:隐藏 6 个周期 **0 次请求**,可见时正常轮询,返回后恢复。

所以 `MonitoringPage` / `OverviewPage` / `AuditBackupPage` 无需改动,
本节新增的 15s 轮询同样自动受管。

RuntimePage 的 `useDocumentVisible` **保留**,但理由不是省请求:
它驱动 `.rt-poll-state` 那个"轮询中/已暂停"指示器,是给用户看的。
第 700 行的 `refetchIntervalInBackground: false` 是在重申默认值,留着无害。

把那段手写 gating 复制到另外四个页面会是 ~30 行纯粹的冗余代码。

---

## 14. 凭据可操作面:一个一直点不到、且流程本身就断的功能

契约 8/9 补的 5 个算子里有 4 个是凭据作用域的。接线之前先查了一遍它们的入口,
结果发现要修的不止是接线。

### 14.1 OAuth 向导以前不可能完成

三件事叠在一起,谁也没发现:

1. **生产里点不到。** `OAuthWizard` 只在 `SubresourcePanel` 里挂载,
   而那个面板走 `graphAvailable()` —— 它等于 `fixturesEnabled()`。
   生产构建渲染的是"等 G1"诚实态,凭据表根本不出现。
2. **拿到了授权地址却不显示。** 契约的 `OAuthOperation` 一直有
   `authorization_url` 和 `failure_class`(7/26 那份就有),
   但向导的本地类型只建模了 `{credential_id, state, expires_at_ms}`。
   用户点"启动授权",界面说 pending 并开始轮询,**却从没告诉他去哪儿授权**。
3. **文案说错了流派。** 写的是"设备授权流",并称 `user_code`/`verification_uri`
   需要契约扩展。实际是授权码流,URL 就在响应里,8/9 还补上了 `oauth/callback`。

**而 fixture 在替它圆谎**:旧 fixture 的 `oauth/status` 在第三次轮询时自动翻成
`complete`,于是 E2E「oauth wizard polls to complete」一直绿。
一个没有完成调用的向导,在一个比网关更宽容的后端上,看起来是work的。

fixture 现在不再自动完成 —— 必须带着正确的 `state` 走 `oauth/callback`。
**fixture 不得比网关仁慈**,否则它测的是一个不存在的系统。

### 14.2 入口:运行时投影就是凭据的枚举

契约没有 `listCredentials`,也没有 `listEndpoints` —— 这正是 G1 要补的。
但 `GET /admin/runtime/availability` 与 `GET /admin/catalog/status`
都是**免 id 的顶层 GET**,且都返回 `{endpoint_id, credential_id, …}`。
RuntimePage 早就把这些 id 渲染在矩阵列头和目录表里了。

所以凭据面挂在运行时页:矩阵列头是凭据的规范清单,点它开详情 Sheet。
`CredentialButton` 各自持有自己的 sheet 状态 —— 两个调用点在不同的表里,
一次只能点一个,页面级选中态是纯粹的管道工程。

**覆盖面的诚实边界**:这两个投影只列出运行时**观测到的**凭据。
建了但从未绑定的凭据,在 G1 之前仍然无处可见。

### 14.3 回调粘贴:客户端先判,别去换一个 400

`OAuthCallbackInput` 要求 `state`,可选 `code` / `error` / `callback_url`。
`state` 是把这次粘贴绑定到本次会话的东西 —— 没有它,网关只能回 400。
`parseOAuthCallback` 因此在前端就把话说清楚:没有 state 是"这不是本次授权的回调地址",
既没 code 也没 error 是"授权可能没有完成",超长是"超出契约允许的长度"。
三条都有单测,E2E 也验证了坏粘贴**根本没有变成请求**。

`authorization_url` 会变成 `href`,所以过 `safeExternalUrl`:
只放行 http/https。"网关发来的"不等于"可以交给浏览器当代码执行"。

### 14.4 顺手补上的全局空缺:锚点从来没有样式

写向导时发现那个授权链接是 UA 默认的 `#0000EE` —— 全仓**没有任何 anchor 规则**,
每一页的链接都在设计系统之外。加了一条 `a { color: var(--tint) }`,下划线保留
(颜色不得单独承载可供性)。

我看截图判断"规则没生效",**结论是错的** —— 实测计算值就是 `rgb(0,113,227)`。
截图上的颜色不可靠,这就是要量的原因。

实测对比度:

| | `--surface` | `--surface-2` |
|---|---|---|
| Light `#0071e3` | **4.70:1** ✅ | **4.39:1** ❌ |
| Dark `#0a84ff` | **4.94:1** ✅ | **4.54:1** ✅ |

浅色下 `--tint` 落在 `--surface-2` 上不过 AA,而那个表面只隔一层卡片。
抬高 `--tint` 不是选项:它同时是导航 ink 与 chart-1 色相,
§10.4 已经证过任何 AA 干净的取值都会塌进 `--ink-2`。

于是加了 `e2e/contrast.spec.ts`:走遍 11 个页面 × 双主题,
把每个链接对**它背后真正那层不透明底色**做测量。
容器型链接(`.count-tile`)刻意跳过 —— 它的内容全是带自己 ink 的子元素,
锚点的颜色不绘制任何东西。我的第一版探针没做这个区分,报了 5 条假阳性。

### 14.5 门禁抓到了我自己

`safeExternalUrl` 的测试里写了 `javascript:alert(document.cookie)`,
`check.mjs` 的 C6 存储禁令扫到字符串字面量里的 `document.cookie` 直接失败。
这是机械门禁的正常代价。**修法是换掉测试载荷,不是给门禁开口子** ——
`alert(1)` 验证的是同一件事(scheme 被拒),而那条禁令的价值远大于一个测试的措辞。

---

## 15. 运营账号池:G1 没来,但它要的东西来了

后端 2026-08-11 收了 P12、开了 **P13「管理与运营」**,首个切片 P13-04A 交付
`GET /admin/operations/account-pools`。这个端点不是 G1,但它一次性解掉了 G1 挡住的两件事。

### 15.1 为什么它比 G1 提案更适合这个面板

它按 **binding** 成行,一行同时携带 provider / channel / account / binding / route:

- **不需要活动版本**。它读的是配置版本,不是运行时快照 —— 草稿就能用。
  这一点决定性:运行时投影(`runtime/availability`)至今返回 `Vec::new()`,
  挂在它上面的入口永远是空的(§14 的教训)。
- **凭据终于可枚举**。`account_id` 就在行里,凭据详情面因此有了真实入口,
  §14.2 里那个"矩阵列头"的临时入口可以退居二线。
- 稳定 keyset 分页 `(provider_id, channel_id, account_id)`,默认 50 / 上限 100,
  游标绑版本与 revision,过期或跨版本 `409`。

代价是两条必须写在界面上的边界:

1. **一行就是一个绑定** —— 建了端点或凭据但没绑定,这里根本不出现。
   面板对零行的空态因此说的是"没有任何绑定",不是"没有子资源"。
2. **投影按设计不返回 URL**(报告原文:no URL/path, ciphertext, digest, headers or body)。
   端点表的「地址」列直接删掉,并写明 `base_url`/`inference_path` 属于配置面。

### 15.2 词汇按后端为准(用户 2026-08-11 决定)

运营面和配置面对同一批实体用了不同的词,而且**不只是命名**:

| | 配置面(既有契约) | 运营面(P13-04) |
|---|---|---|
| 实体 | upstream / endpoint / credential | provider / channel / account |
| 凭据状态 | active / disabled / **revoked** | active / **cooling** / **unauthorized** / disabled |
| 传输 | **https** | **http** / sse / websocket |

第三行不是纸面差异:实测同一个端点,配置面建的是 `"https"`,运营面回的是 `"http"`。

用户的裁决是**以后端为准**。落地为一条规则:**哪个端点回答,就用哪个端点的词** ——
运营面的表头写 `channel_id`/`account_id`,状态徽章直接显示 `cooling`,
`transport` 原样显示 `http`,前端不发明映射层。这与既有的
"契约值(错误码、枚举、标识符)永不翻译"是同一条纪律(§9 rule 16)。

`cooling` 与 `unauthorized` 各给一个独立色调:一个是"等",一个是"停",
折成同一个会让运维分不清该等还是该处理。

### 15.3 `configured_enabled` 是静态合取,不是健康

报告的 explicit non-claims 写得很清楚:`enabled` = `provider && channel && binding`,
**不代表**凭据 active、健康、有额度或当前可路由。

一个绿色的 `enabled` 徽章很容易被读成"这条路能用"。所以绑定表下面直接写了这句话,
并指明运行时状态要等 P13-06 的 Provider 池投影。这不是啰嗦 —— 这是唯一能防止
"库存显示 enabled,请求却全失败"变成一次误判的地方。

### 15.4 顺手抓到的既有布局 bug

`.row-actions` 是 `display: flex`,而它加在 `<td>` 上。
flex 会把单元格移出表格布局,浏览器于是把连续的 flex 单元格包进**一个匿名单元格** ——
两个相邻的 `.row-actions` 列因此叠在一起。

实测证据:修复前两个 `<td>` 都报 `x=1140 w=174 h=46`(行高 93 的一半);
修复后 `x=1139` 与 `x=1313`,高度都是 47。

这个 bug 在旧面板(端点表同样有「测试」+「目录发现」两个相邻 `.row-actions`)就存在,
只是从没被测量过。修法是让 `td.row-actions` 保持 `table-cell`,按钮改用行内间距。

### 15.5 提案通道少了一半

`proposed.ts` 原本承载 G1 graph 与 G3 analytics 两条未落地契约的 DEV 专用通道。
G1 那条已删除(连同 `upstreamSubresources` 与它的测试)—— 真端点到了,
留着假的只会让人以为还有第二条路。analytics 那条还在,等 P13-05。

---

## 16. 访问组与路由授权:配置链上缺失的一半

盘点契约用量时发现:76 个算子里 prism 只调用 41 个,而一个可用配置需要的 12 步里
**面板只覆盖 6 步**。今天演示用的那份配置图,全部是用 curl 灌进去的 ——
面板做不到,这就是证据。

访问组是其中最刺眼的一环:表格自始至终没有「操作」列,`grantAccessGroupRoute`
完全没有 UI。没有它们,签发出来的 Client Key **到不了任何模型**。
而这四个算子从第一天起就在契约里,没有被任何东西阻塞过。

### 16.1 限额编辑:显示格式就是输入格式

`AccessGroupInput.limits` 是自由对象(`Record<string, integer ≥ 0>`,最多 16 项)。
表格本来就把它渲染成 `key=value key=value`,于是编辑器直接收同一个字符串 ——
运维可以把表格里那一行原样复制回表单,不需要脑内翻译。

`parseLimits` 在前端就把契约会拒绝的东西拦下来(负数、小数、非整数、重复键、超过 16 项),
理由和 OAuth 回调粘贴一样:与其换一个 400,不如当场说清哪里不对。

### 16.2 PATCH 是整体替换,所以表单必须预填

`PATCH /admin/access-groups/{id}` 收的是**完整的 `AccessGroupInput`**,不是 partial ——
这一点我在灌演示数据时先踩过一次(部分 body 直接 400)。

后果是:编辑表单若不预填当前值,保存就会把没碰过的字段悄悄清空。
E2E 因此显式断言三个字段都带着现值打开,并在表单里写明「保存等于整体替换」。

### 16.3 路由不可枚举

契约有 `createRoute`/`getRoute`/`updateRoute`/`deleteRoute`,**没有 `listRoutes`**。
授权表单因此不能给一个下拉框。

折中:自由文本 + datalist 建议,建议来源是运营库存里出现过的 `route_ids`,
并在表单里直说「建议并不完整」。这比一个假装完整的下拉框诚实,
也比纯文本框好用。

没有授权的组不显示空表格,而是直接说「组内的 Client Key 现在到不了任何模型」——
空表格会被读成"正常",这句话不会。

### 16.4 浏览器 origin 被默认拒绝 —— 嵌入切换不是可选项

实机验证时,GET 全部正常,而 **POST 一律 `404 management_access_denied`**。

原因不在 prism:`management_security.rs` 开头就写着
"Browser origins are denied by default",而 `deployment.rs::management_origin`
把允许的 origin **硬性推导为管理监听器自己的地址**:

```rust
// apps/gateway/src/deployment.rs:576-582
fn management_origin(listener: SocketAddr) -> Result<ManagementOrigin, DeploymentError> {
    let value = format!("http://{}:{}", address.ip(), address.port());
    ManagementOrigin::try_new(value)…
}
```

也就是说:**只有从管理监听器自己提供的 UI 才能做写操作**。任何反代、任何独立端口的
托管方式,浏览器带上的 Origin 都不匹配,所有 mutation 全部拒绝。

这把嵌入切换(`web/admin-ui/dist` 四个文件名对齐)从"交付上的收尾"
升级成 **生产里写操作能工作的唯一前提**。演示用的反代因此重写 Origin 头,
以复现嵌入后的条件 —— 那是复现,不是绕过。

### 16.5 实机结果

对真网关(`serve` + 真库)从面板完成:新建访问组 `ag-live`
(`max_concurrency=6 rpm=300`)→ 授权路由 `rt-grok4`,零失败请求,
并经 API 直接确认落库。这是 prism 第一次真正写出一段此前只能用 curl 造的配置。

---

## 17. 路由候选与 Route Explain(2026-08-18)

### 17.1 补的是一个断口,不是一个功能

面板此前能建路由,而且只能建路由。零候选的路由被后端两条独立路径拒绝:

```rust
// crates/gateway-control/src/management_mutation_service.rs:2074
if active_candidates.is_empty() {
    error_codes.push("route_missing_active_candidate");
}
```

(第二条是 `route_compiler.rs:1254`,发布时的编译路径。)

于是每一条在 Prism 里建出来的路由,都把草稿留在**面板自己修不回来**的状态:
validate 失败 → 发布被挡 → 界面上没有任何入口能加候选。运维只能回滚或改用 curl。

更糟的是当时的成功提示写着"候选编辑等待 G1 契约解锁",而 `createRouteCandidate`
一直在契约里。**那句话把前端的欠账说成了后端未交付**,谁读了都会去等一个不会来的东西。

### 17.2 三条契约事实,写在界面上而不是绕开

1. **没有 `listRoutes`。** 全部 99 个算子里唯一能枚举 route_id 的读操作是
   `listAccessGroupRoutes`,其次是运营库存的 `route_ids` 字段(本页用后者,与
   AccessPage 同源)。两者都不完整,而**刚建好、还没有候选的路由两边都没有** ——
   那恰好是要来修的那一类。所以输入框保持自由文本,并把这句话印在旁边。

2. **候选只能新增。** `createRouteCandidate` 存在,没有 list / update / delete。
   唯一的读取路径是 `explainRoute`,而它需要一个请求模型和协议才能作答。
   表单直说"写错了只能删掉整条路由重建",不暗示候选可以改回去。

3. **validate 只查草稿拓扑。** 后端自己的注释:"Full compiler/capability
   admission remains the later publication boundary."所以校验区底部固定写着
   **"这里通过不等于发布会通过"**。绿色对勾很容易被读成发布许可,那不是它的意思。

### 17.3 `capability_override` 为什么是自由文本而不是复选框

契约是 `{type: object, maxProperties: 32, additionalProperties: boolean}` ——
**限制的是值的类型,不是键集**。用本面板的 `SEMANTIC_CAPABILITIES` 做复选框网格,
会悄悄拒掉后端接受的键。`key=true key=false` 的解析器沿用 `parseLimits` 的形状
(显示格式 == 输入格式),键任意,并且 `vision=1` 报错而不是强转成 `true`。

### 17.4 Explain 的两处静默漂移

**`PROTOCOLS` 少一个值。** 契约的 explain 协议枚举是三个,前端只列了两个 ——
`openai_chat_completions` 随 P12-08 进契约后,前端从未跟进。漂移门禁看不见
**页面没写出来的字面量**:它比对的是 vendored 契约与生成客户端,不是页面提供的选项。
后果是 Chat Completions 这条路径在面板里根本无法解释。

**响应新增必填字段,页面一声不吭。** P13-07B/D 给响应加了必填 `price_policy`
与每候选必填 `price_evidence`,并加了可选 `provider_id`(多 Provider 路由省略时
按契约 fail closed)。页面只**读**这个类型、从不构造它,所以 `tsc` 全绿。

这次改动里,同一个字段的缺失**在单测里立刻炸了**——测试要构造 `ExplainCandidate`,
必填字段少一个就编译不过。**可构造 = 可检测**,只读的调用点没有这个保护。

### 17.5 `.sheet-panel` 从来没有 max-height(既有缺陷)

候选表单有 9 个字段,比视口高。`.sheet-backdrop` 是 `place-items: center` +
24px padding,**面板超出视口就两头被裁,提交按钮永远够不着** ——
Playwright 报 `element is outside of the viewport` 重试 51 次。

修在共享层:面板封顶 `calc(100dvh - 48px)`,滚动的是 `.sheet-form` 而不是面板
(`.glass` 的 `::before` / `::after` 是面板内的绝对定位,让面板滚会把材质一起卷走),
动作行 `position: sticky; bottom: 0`。CredentialSheet 也在这条线附近,只是没人撞上。

### 17.6 实机验证:一半成立,另一半做不到,原因清楚

对真网关(全新 state-dir、种子配置经真 API 写入、从 `/admin-ui/` 打开)跑完整闭环:

```
NOTICE     路由 rt-853903 已创建,但它还没有候选 …
VALIDATE#1 valid=false · route_missing_active_candidate
VALIDATE#2 valid=true          ← 加完候选
PROBLEMS   []
```

**Explain 在真网关上返回 503,验不了。** 原因不是"没接线":

```rust
// apps/gateway/src/runtime.rs::explain_route
let snapshot = self.snapshot_for(request.config_version_id())?;   // ← 第一步
```

Explain 对**已编译快照**求解,而快照只在版本**发布后**存在。草稿上它 503,
与"本部署未接线"在协议层完全无法区分。而离线部署发布不了,所以 A2 的价格证据渲染
**只在 fixture 下验证过**,这一点不含糊。

两个直接后果:

- **文案修正。** 面板知道当前版本是不是 active,所以草稿上的 503 现在说
  "草稿版本没有可解释的快照 · 先发布该版本,或改选一个 active 版本",
  不再让运维去查一个不是他的问题。
- **fixture 修正。** fixture 原本对草稿返回 200,等于让"草稿上 Explain 能用"
  这个不存在的状态看起来正常 —— 与当年 OAuth fixture 自动完成、
  把一个没有 completion 调用的向导演成可用的,是同一类谎。现在 fixture 对
  非 active 版本返回 503,E2E 因此拆成两条:草稿测文案,active 测价格证据。

### 17.7 一处仍未机械化的盲区

`sync-contract` + `check.mjs` 保证 `contracts/` 与 `src/generated/` 跟契约一致,
但**响应体新增必填字段时,只读的调用点不渲染它,类型检查与门禁全部照过**;
**页面漏掉一个 enum 字面量**同理不可见。17.4 两处都是从这个缝里漏出来的。

目前只能靠读 `docs/cross-boundary-log.md` 的 action-required 条目补。
可考虑的机械化方向(尚未做):让 `check.mjs` 比对"契约响应必填字段名"与
"src 中出现过的字符串",给**警告级**提示 —— 不阻断,因为字段名可以被解构改名,
误报会比漏报更快让人把门禁关掉。

---

## 18. 用量分析页重建(2026-08-18)

### 18.1 这不是改造,是替换

旧页面按**提案中的 G3 分析形状**建成:六个 tab、时间桶、热力图、缩放刷。
它的数据源是 `api/proposed.ts`,而那个模块的开关是:

```ts
export function analyticsAvailable(): boolean {
  return import.meta.env.DEV && import.meta.env["VITE_PRISM_FIXTURES"] === "1";
}
```

**生产构建恒为 `false`。** 也就是说这一族代码从未在真网关上渲染过任何像素 ——
它一直显示 "contract pending" 空态。后端最终实现的是 `operations/usage`,形状完全不同。

替换后:usage 一族 1716 行 → 约 730 行;连带清掉六个只为时间桶而存在的图表组件
(Heatmap / ZoomBrush / LineChart / MultiLineChart / SeriesLegend / RankTable)
与三个只为它们服务的辅助模块,共 **-2797 / +1196 行**。

**没有发布字节收益可宣称:**重写 UsagePage 后这些组件已被 Vite 树摇掉,
删除去掉的是源码而不是 `dist`。产物在删除前后逐字节相同。

### 18.2 契约的三条属性决定了页面长什么样

**没有服务端时间桶。** 一行 = 一个 7 元组在整个 `[from_ms, to_ms]` 窗口内的聚合,
`observed_at_ms` 是行上的水位而不是桶戳。所以**没有趋势线、没有热力图、没有缩放**。

前端拼一条曲线的两种做法都不可接受:发 K 个窗口 × 每窗口跟游标 = K×页 次请求;
不跟游标就静默少算。页面把这句话印出来,而不是画一条看起来很像的曲线。

这也让 ECharts 的引入条件永久失效 —— 计划里"等时间桶到位"的那个前提不会满足。

**`limit` 上限 100。** 任何真实部署都要翻页,所以**单页求和就是错的**。
页面跟游标读到底,上限 `MAX_PAGES = 20`,到顶时明说"下面的合计是不完整的"。
E2E 用 137 行 fixture 钉死这一点:全量合计 1,225,只算第一页是 885 ——
两个数差得足够远,回归一眼可见。

**六个 token 家族各带独立置信度,`total` 可空。** `null` 是"未观测",不是零。
`sumFamily` 因此:null 贡献者不计入求和、把置信度压到 `unknown`、并置 `partialCoverage`。
UI 用 `≥` 标记这类合计是**下界**。把 null 当 0 相加,会得到一个精确的错数。

### 18.3 一处漏掉会让人误读的事实:usage 不受配置版本影响

`listOperationalUsage` **不声明 `X-Config-Version`**。核对下来,运营面的算子分成两类:

| 版本作用域 | 无版本作用域 |
|---|---|
| `listOperationalAccountPools`(配置库存) | `listOperationalUsage` |
| `listProviderEgressStatus` | `listOperationalBilling` |
| `listBillingCatalogs` | `listProviderAccountPools` |
| | `listRequestAttempts` |

用量是**已发生请求的持久观测**,天然跨版本;配置库存则必须绑版本。

第一版我照着别的页面加了 `versionScoped: true` 并要求先选版本,fixture 也照做,
结果真网关上一测就崩(`unknown config version`)。现在页面不要求选版本,
并在正文里写明**顶栏所选版本不会过滤本页数字** —— 不写的话,
运维会以为选了草稿就只看到草稿的用量。

### 18.4 又一次被自己的门禁抓住

`check.mjs` 禁止内联 style 属性(生产 CSP 是 `style-src 'self'`),用的是文本匹配。
我在注释里**写出了那个被禁的字面串**来解释为什么用 SVG,于是门禁报了自己写的注释。

与当年 `document.cookie` 写在测试字符串里触发 C6 是同一类。处理方式也一样:
**改文案,不改门禁。**门禁宁可误报也不该被削弱。

### 18.5 实机验证与其边界

对真网关(`/admin-ui/`,**故意不选任何配置版本**)验证通过:
`GET /admin/operations/usage` 返回 200、页面不索要版本、空态说明"从未接过流量"、
水位显示"尚无观测"、零控制台错误。

**非空渲染只在 fixture 下验证过。** 真网关是全新库、没有流量跑过,而要产生真实用量
需要发布配置并真的发请求,离线部署做不到 —— 与 §17.6 里 Explain 的限制同源,
都是"没有已发布快照"这一个前提。

---

## 19. 请求监控页重设计(2026-08-18)

### 19.1 为什么是重设计而不是接线

旧页面有一行 KPI:**P50/P95 延迟、成功率**,外加一张"实时事件流"表。
这三样在交付的契约里**一样都没有**,所以它不能被"接到新数据源上" —— 没有对应物。

能诚实拿到的只有三条,而且三条的作用域各不相同:

| 源 | 有什么 | 版本作用域 |
|---|---|---|
| `listOperationalBilling` | 每条**已计费请求**:六类 token、成本、计价置信度 | **否** |
| `listProviderAccountFailures` | 每条**归因到账号的失败尝试**:错误码/归因层/重试决策 | **是** |
| `listRequestAttempts` | 单请求的尝试轨迹(裸数组,无分页) | 否 |

**没有一条带延迟。**

### 19.2 两条流不是同一个总体的两半

账本装的是产生了用量记录的请求;失败流装的是归因到某个账号的**尝试**。
一次请求可以同时出现在两边、一边都不出现,或在失败流里出现**多次**
(每次重试一条)。

**用它们相除得出的"成功率"是编的。** 页面把这句话直接印在顶部,
因为这正是下一个接手的人最可能顺手做的事。

失败面板的行数也因此标注为"已加载失败尝试",而不是"失败请求数"。

### 19.3 `status` 参数不是状态

契约里 `listOperationalBilling` 的查询参数叫 **`status`**,取值却是
`exact|partial|unknown|unpriced` —— 是**计价置信度**,不是请求成败。

参数名邀请的正是错误的读法。界面上这个控件叫「计价置信度」,
E2E 里有一条断言钉住"筛选区不得出现『状态』二字"。

### 19.4 summary 覆盖整个窗口 —— 这是核实过的,不是假设

页面显示"账本记录数 / 成本精确占比 / 已知成本",而只加载了第一页。
这合法当且仅当 `summary` 不是页汇总。查了后端:

```rust
// gateway-control/src/management_operations_service.rs
let mut summary = OperationalBillingSummary::default();
for entry in &filtered { … }          // 全量筛选集
if let Some(cursor) = &query.cursor { filtered.retain(…) }   // 游标在之后
filtered.truncate(query.limit);
```

累加跑在**游标与截断之前**,且由 `snapshot_ledger_id` 钉住快照。
所以第一页的 summary 就是整窗口的答案。

失败流**没有**这样的汇总,所以它的分布明确标注为"只统计已加载的这些行"。
同一页上两种不同的诚实,不能混用一套话术。

### 19.5 microunits 没有币种

`cost_microunits`、`input_microunits_per_million` —— 契约从头到尾**没有声明币种**。
所以页面与 JSONL 导出都只写 `microunits`,不折算、不加 `$`/`¥`。
单测里有一条断言导出行不含任何货币字样。

### 19.6 又一次自触发门禁(第三次)

E2E 断言"页面不得出现 P95 / 成功率"失败了 —— 因为**免责声明本身写了这两个词**。

处理:负向断言只盯**呈现数据的地方**(`.mon-summary` 与 `.mon-table thead`),
声明里提名字恰恰是对的。这与 §18.4(注释触发 CSS 门禁)、
C6 那次(`document.cookie` 写在测试字符串里)是同一个模式:
**文本门禁会抓到讨论它自己的文字。**每次的正确处理都是改断言范围或措辞,而不是放宽门禁。

### 19.7 顺带修掉一处被 B2 打断的链接

Overview 上"在请求监控中查看 →"指向 `/monitoring?range=today&status=failed`。
两个参数在新页面都无意义:`range` 不存在,`status` 是计价置信度而非成败。
已改为指向失败归因页签 —— 那才是"最近失败"的诚实去处。
Overview 自身仍跑在 `proposed` 上,整页重建是 B4。

### 19.8 实机验证

对真网关(`/admin-ui/`)验证通过:

- 账本页签**不选任何配置版本**即返回 200,空态与 `—` 占位正确(`records=0` 时精确占比是 null 不是 0%);
- 失败页签在无版本时明确说明"需要一个配置版本"并指出账本不需要;选版本后正常查询 200;
- 零控制台错误。

非空渲染同样只在 fixture 下验证 —— 真库没有跑过流量,产生账本行需要真实请求。

---

## 20. 计费与价格目录页(2026-08-18,全新)

### 20.1 一页两种作用域,这是本页最重要的一件事

| | 作用域 | 依据 |
|---|---|---|
| **价格目录** | **全局** | `list_billing_catalogs_bounded()` 不接受版本参数;`X-Config-Version` 只用于带回 revision |
| **路由价格策略** | **按配置版本** | 策略行写在所选草稿上,`upsert_routing_price_policy(config_version_id, …)` |

也就是说:**在草稿上导入一份目录,所有配置版本立刻都看得到**。
"我在草稿里操作所以是隔离的"是这一页最可能出现的误判,页面因此把这句话写在目录卡片顶部。

### 20.2 导入是整份提交,而且只增不改

契约里没有目录的修改与删除算子。改价的做法是**导入一份新目录**;
撤销的做法是**回滚出一份新目录**(复制旧条目、向前追加)。`rolled_back_from` 记录血缘。

`entries` 的 `minItems` 是 **1** —— 空目录非法,所以"清空价格"不是一个可表达的操作。

### 20.3 512 条不是用表单填的

条目上限 512,来源是计价表导出。所以导入口是**粘贴 JSON**,由
`parseCatalogEntries` 按契约边界严格校验,并**报出第几条、哪个字段**:

> 第 2 条:model 必须是非空字符串。

在 512 行的粘贴上返回一句 `400 invalid_management_request` 对拿着计价表的人毫无用处。
校验在前端做完才发请求,E2E 里有一条断言"请求根本没发出去"。

顺带:任一条目重复 `provider/channel/model` 也在本地拦下 —— 后端有自己的判断,
但一次明显的粘贴重复不值得走一趟网络。

### 20.4 未生效的目录不能绑定

`set_routing_price_policy` 检查 `catalog.effective_at_ms > now` 并以
`RoutingPriceCatalogNotEffective` 失败关闭。所以绑定选择器**只列已生效的目录**,
未生效的在列表里带「未生效」徽章并压低对比度 —— 让人从 4xx 里发现这件事是糟糕的设计。

### 20.5 404 是状态不是错误 —— 但不能只看 404

未配置策略时 `getRoutingPricePolicy` 返回
`404 management_resource_not_found`。这是**合法状态**:它正是所有候选的
`price_evidence` 读作 `disabled` 的原因,不该画成红色错误。

**但判定必须连错误码一起看。** `classifyStatus` 里:

```ts
if (status === 404 && code === "management_access_denied") return "session_invalid";
```

`404 management_access_denied` 是网关对**不被允许的浏览器 origin** 的失败关闭应答,
客户端会据此重置会话。若把"任何 404"都当作"策略未配置",就会在一个正在死去的会话上
画一个平静的空状态。单测里专门有一条钉住这个区分。

### 20.6 清除策略要说清后果

清除不是"关掉一个开关":本版本**每一个候选**的 `price_evidence` 都会变成 `disabled`,
基于费率的路由比较随之停止。确认框把这句话写全,并说明目录本身不受影响。

### 20.7 实机验证 —— B 批第一个完整跑通写循环的页面

用量与账本页都受限于"离线部署产不出真实流量",非空渲染只能靠 fixture。
**计费页不依赖流量**,所以在真网关上跑完了整条写循环:

```
HTTP 404 GET    /admin/billing/routing-price-policy   ← 渲染为"未配置"状态
HTTP 200 GET    /admin/billing/catalogs
HTTP 201 POST   /admin/billing/catalogs               ← 真导入
HTTP 200 PUT    /admin/billing/routing-price-policy   ← 真绑定
HTTP 204 DELETE /admin/billing/routing-price-policy   ← 真清除
HTTP 404 GET    /admin/billing/routing-price-policy   ← 回到"未配置"
PROBLEMS >>> []
```

---

## 21. Overview 收口与影子通道的终结(2026-08-18,批 B4 + B5)

### 21.1 最后一块死代码

Overview 的下半页是按提案的 G3 分析形状建的:今日 KPI、按小时趋势、健康条带、
模型排行、延迟分位。生产里它整块渲染成一张"事件管道尚未接线"的卡片。

替换它的东西刻意很小,因为这一页上**只有两样东西能既诚实又便宜地给出**:

- **计价可信度**:`listOperationalBilling` 自带的 summary 覆盖整个账本窗口
  (§19.4),所以 `limit=1` 的一次请求就能得到准确 KPI;
- **其余都需要跟游标读到底**。用量分析页会那样做并在提前停止时明说 ——
  在总览放一个"只读了一页"的近似值,会和那一页直接打架,所以这里放**指路**而不是数字。

延迟与成功率仍然哪里都没有,页面把这句话连同原因一起写出来。

### 21.2 `analyticsAvailable()` 的最终账

```
删除:  api/proposed.ts · proposed-types.ts · proposed.fixtures.test.ts
        components/data 里 8 个组件 + 3 个辅助模块
        dev/fixtures.ts 里 191 行分析端点
        usage / monitoring / overview 三页的分析半区

components/data:  1243 行 → 141 行(只剩 SparkLine / StatTile / TokenMixBar)
生产死代码:      3479 行 → 0
```

### 21.3 门禁:让这件事不能再发生一次

删掉代码不解决问题 —— 问题是"契约没有的形状,可以在 src 里长出一条只在 dev 下应答的通道"。
`check.mjs` 因此新增一条:

> `src/**` 不得 import `api/proposed` 一类的提案端点通道。
> 契约是端点的唯一来源;形状缺失时走 `docs/change-requests/` 加诚实空态。

**门禁写完必须验证它真的会响。** 临时放一个违规文件进去,确认 FAILED,再移除确认恢复 OK ——
一条从未失败过的门禁和没有门禁是一回事。

### 21.4 第四次自指,这次是预先避开的

这条门禁的正则要求 import 上下文(`from "…api/proposed"`),而不是裸字符串匹配。
原因是 §18.4 / §19.6 / C6 那三次教训:**文本门禁会抓到讨论它自己的文字**。
`metrics.ts` 的注释里就写着 "Lived in api/proposed-types until…" ——
如果按裸串匹配,这条注释会让门禁在自己落地的那一刻就红。

### 21.5 实机验证

真网关上 Overview **整页要么是真数据、要么是诚实空态**,第一次没有任何"等待某个未来契约"的卡片:
活动版本、布线规模、实时计数器(Prometheus)、观测管道健康、计价可信度(账本 summary)、
以及一张说明为什么没有趋势线并指向真正能回答问题的两页的卡片。零控制台错误。

---

## 22. Provider 账号池 · 实时(2026-08-20,批 C1)

### 22.1 又一处作用域分裂,而且这次是"看得见点不动"

| 算子 | 版本作用域 |
|---|---|
| `listProviderAccountPools` | **否** —— 实时状态 |
| `applyProviderAccountPoolAction` | **是** |

所以未选版本时表格照常渲染,而每个操作按钮都不可用。RuntimePage 原本在没选版本时
整页早退成一句"先选一个配置版本" —— 那句话对这张表是**假的**,现在改成先渲染池卡片,
再解释其余三个投影为什么需要版本。

另外这个 action **没有 `If-Match`**:它是版本作用域但不受 revision 保护,
因为它动的是运行时而不是配置。

### 22.2 认证与运行时是两个轴,不合成"健康"

`auth_status`(4 值)与 `runtime_status`(7 值)各自独立:一个账号可以认证正常而运行时
正在冷却,也可以认证已过期而运行时尚未察觉。合成一个健康值等于发明一个后端从未报告的状态。

单测钉住了这一点的一个具体后果:`authStatusMeta("cooling")` 必须是「未知」——
cooling 不是认证轴的成员,跨轴取值应当被当作未知而不是"碰巧能查到"。

同一轴内也分清等待与停止:`cooling` 会自己恢复(warn),`unauthorized` 不会(critical)。

### 22.3 `rejected` 是答复不是失败

202 回执有四态。`rejected` 表示调度器**拒绝了这次操作** —— 这是一个答案,
不画成错误;`recovery_required` 表示自动恢复不适用,需要人工。二者文案分开。

409 陈旧目标则重新读取快照并说明,而不是盲目重试。

### 22.4 原生校验先于自写校验

冷却时长的输入带 `min`/`max`,所以越界值被**浏览器自身的约束校验**在提交前拦下,
`validCooldown` 根本不会执行。E2E 因此断言的是 `validity.rangeUnderflow` 与
"sheet 仍然打开、什么都没发出去",而不是断言一条应用级错误 ——
断言一个不会发生的错误,等于测试一条死路径。`validCooldown` 仍然保留,
它防的是不经过这个输入框的值(单测覆盖)。

### 22.5 一处后端不一致(已记录,未改)

真网关上 `listProviderAccountPools` 在**未接线时返回 500**:

```rust
// management_resources.rs:8071
ProviderAccountPoolError::InvalidSnapshot | ProviderAccountPoolError::SourceUnavailable => {
    internal_error()   // ← 500
}
```

而这个网关里**其余所有注入式投影未接线时都是 503** —— 那正是 `isProjectionUnavailable`
的判定依据,也是 `UnavailableBlock`(「此部署未启用该投影」)的触发条件。

后果:未接线的账号池在面板上读作"读取失败 · Management operation failed",
运维会去查一个不是自己的 bug。

**前端不冒充判断**:500 确实也可能是真的内部错误。所以错误块把两种可能都写出来,
并注明"本投影未接线时也返回 500(其余投影用 503)"。已在跨界日志记录。

### 22.6 又一次踩到嵌入包陈旧

实机验证第一次失败,因为我在最后一次 `cargo build` **之后**才改的源码 ——
网关里嵌的还是旧 bundle。**改完前端必须重新 `cargo build` 才能实机验证**,
这条在 §17.6 之后又犯了一次,记在这里。

---

## 23. Provider 出口状态 · 三个域(2026-08-21,批 C2)

### 23.1 一次混读会让某个域"看起来是空的"

契约把 `egress` / `session` / `clearance` 三种行放进**同一个分页流**,共用一个 cursor。
最省事的做法是读一页再在浏览器里按 `domain` 分区 —— 但那样一台有 100+ 条 egress 行的
部署,第一页里会**一条 session 行都没有**,而本页的空态写的是"该来源不存在"。

于是分区展示就不再只是版式问题:**它决定了空态那句话是真是假。**
所以是三次独立读取,每次带 `domain=`,而不是一次混读再切分。

代价是三个快照而不是一个 —— 它们可能不同步。这一点不藏起来:每个分区打印自己的
`snapshot_id` 与采样时刻,卡头也直说"三个分区各自读取,快照可能不同"。

### 23.2 两种 409,两种不同的补救

| code | 含义 | 补救 |
|---|---|---|
| `..._cursor_conflict` | 运行时快照在游标下轮换了 | 本区**从头重读** |
| `..._config_conflict` | 所选版本不是这份快照的来源 | 从头重读**没用**,要换版本 |

计划原文把 409 当成一件事("快照冲突后从头重读")。它是两件事,给出同一句提示会让
第二种情况下的操作员反复点一个永远不会成功的按钮。

游标冲突**不自动重读**。已经读到的行留在屏幕上(它们在读到的时刻是真的),
错误提示旁给一个显式的「从头重读」。静默换掉一屏行,比多点一下更糟。

### 23.3 一个共享层的错报:409 不等于"配置被改了"

`client.ts` 原本对**任何** 409 调用 `markConflict()`,而外壳的横幅写的是
「配置已被其他会话修改」。全契约十个 409 code 里有**五个**是运行时侧的:

```
management_operations_cursor_conflict
management_provider_account_pool_cursor_conflict
management_provider_egress_status_cursor_conflict
management_provider_account_action_target_changed
management_channel_pin_target_changed
```

这五种情况下没有人改过任何配置。弹这条横幅等于让运维去查一次并不存在的变更。

前三个**今天就能触发** —— 用量 / 监控 / 计费页都在翻分页。所以这是本批之前就存在的
缺陷,不是 C2 引入的;修在 `errors.ts::isRuntimeConflict` 这一个地方,五条调用路径一起好。
`..._config_conflict` **故意不在名单里**:它确实是"所选版本不再是来源",横幅是对的。

E2E 加了一条断言:游标冲突后 `.conflict-bar` 必须为 0。已用临时回退验证过它确实会失败 ——
没验证过会失败的门禁等于没有门禁。

### 23.4 三套状态词汇不能合并成一张表

后端是**一个** 14 值闭集,但按域显式校验兼容性:`fresh` 是 clearance 的状态,
egress 行永远不会带它。前端照抄这个校验 —— 三张 map,跨域取值一律「未知」,
与 §22.2 认证/运行时两轴同一条纪律。合并成一张查找表会让一个后端本会拒绝的行渲染得像合法的。

`probe_due` 的标签特意写成「可启动探测」而不是「可探测」:它是**许可**,不是已经恢复,
而后者读起来像健康。

### 23.5 `target_kind` 与 `target_id` 是各自独立可空的

所以"具名出口但没报告 id"是契约允许的一行,而它**不等于直连**。空单元格会抹掉这个差别,
`formatTarget` 因此把它渲染成「具名出口(未报告 id)」。

### 23.6 实测:503 在这里有两种可能

一台刚启动、建了配置版本但没导入任何 Provider 凭据的网关,三个域全是
**503 `management_runtime_unavailable`**。共享的 `UnavailableBlock` 把 503 读成
"这台部署不提供该投影",而这里还有第二种可能:接线正常,只是没有可投影的来源快照。

两者的处置不同(一个找运维,一个导凭据),所以卡片在 503 时多加一句把两种可能都写出来。
**面板不猜。**

注意这一条与 §22.3 的账号池不同:那里的问题是后端用 500 而非 503,属于不一致;
这里 503 是对的,只是原因不止一个。

### 23.7 本批的验证边界

真网关上验到的:三分区渲染、版本作用域生效、503 走"投影未启用"而不是错误弹窗、
无 action 按钮、无 overall health、外壳不弹配置冲突横幅。

**没验到的:任何一行真实数据。** 当前来源只覆盖已组合的 Grok Build / Console 运行时状态,
离线部署导不了 Provider 凭据。行渲染、翻页与游标冲突恢复**只在 fixture 下验证过** ——
与 §17.6 的价格证据同一类边界,写在这里而不是含糊过去。

---

## 24. 兼容出口 · 代理池 / 节点 / 绑定(2026-08-21,批 C3)

### 24.1 这个只写字段的更新语义与凭据密钥**相反**

| | `CredentialInput.secret` | `CompatibleProxyNodeUpdateInput.proxy_endpoint` |
|---|---|---|
| 创建时 | 必填 | 必填 |
| PATCH 时 | **必填** —— 只改状态也得重输 | **可省略/可 null** —— 省略即保留 |
| 读模型 | `secret_present` | `proxy_configured` |

计划里写的是"与 `CredentialInput.secret` 同一类诚实处理"。**同类,但不同向。**
两者都只写、都不回显 —— 到这里为止一样;但把 Account 表单那句"哪怕只想改状态也必须重新输入"
抄过来,就是在告诉运维去重打一个正在正常工作的代理地址。契约原文是
"Omitted or null preserves the existing sealed endpoint; a string rotates it"。

所以节点表单在编辑态明说**留空表示保留**,并且只在operator真的填了东西时才做校验 ——
空值不是"待校验的空",是"不改"。

### 24.2 `proxy_configured` 是常量,不是观测

```rust
// management_mutation_service.rs:378
proxy_configured: true,
```

后端硬编码为 `true`:落库的节点必然有一个封存的地址。所以这一列**永远不会**显示"未配置",
它不是一次检查的结果。界面因此不能让它读起来像"面板验证过这个代理可用" ——
它只说明"库里有一个封存值",与那个代理是否可达毫无关系(E5 真实网络仍未授权)。

三元渲染保留,因为契约声明的是 boolean;真有一天返回 false,写死"已配置"就是撒谎。

### 24.3 `target_id` 来自两个不同的命名空间

后端按**整对**匹配:

```rust
("direct", None) | ("fixed_proxy", Some(node)) | ("proxy_pool", Some(pool))
// 其余一律 400
```

即 `direct` + 任意 id 与 `proxy_pool` + 无 id **同样非法**。而线上两者都只是"一个 id 字符串",
填错命名空间得到的是一个读不懂的 400。所以表单按 `target_kind` **切换候选列表**
(节点列表 / 池列表),`direct` 直接**不渲染这个字段**。

### 24.4 删除没有级联,所以提前预测

后端对被引用的池 / 节点直接拒绝。两个列表本来就在屏幕上,于是确认框直接点名谁在引用
(`节点 node-eu-1`、`绑定 ep-.../cred-...`),而不是让 operator 点下去换一个失败请求。
fixture 也照同样规则拒绝,否则"预测"就没有任何东西可对照。

### 24.5 新建按钮在**分区级**,不在行级

一个没有节点的池没有任何行。而**刚建出来的池正是这个样子** —— 如果"新建节点"挂在行上,
你刚创建的东西就成了唯一一个加不了节点的池。`groupNodesByPool` 因此显式保留空池,
并写一句"这个池还没有节点"。这是子资源 CRUD 那次教训的直接落地。

### 24.6 实机验证抓到的两个问题

**第一,fixture 模式下 `page.on("request")` 什么都抓不到。** fixtures 走
`options.fetch` 注入,请求根本不到网络层。我最初两条断言
(「没有发出 POST」「PATCH body 里没有 proxy_endpoint」)因此**恒真** —— 是假通过。
改成断言可观测的事实:失败提交后**那一行不存在**;空地址保存**成功**本身就是证明,
因为 fixture 与网关一样会拒绝空字符串,真发了空值就会 400。两条都用临时回退验证过会失败。

**第二,`th { text-transform: uppercase }` 把 id 变成了另一个字符串。** 实机输出里
`node-live` 显示成 `NODE-LIVE`。列标题该大写,**行头是 id,而 id 大小写敏感** ——
照屏幕重打一遍 `NODE-EU-1` 指向的是不存在的资源。运行时矩阵早先为行标签修过同一处
(`.rt-rowhead`),这里补 `.cp-rowhead`,并加了一条断言钉住 `text-transform: none`。

### 24.7 三个 `getCompatible*` 单资源 GET 故意没接

与 `getEndpoint` 不同 —— 那个必须接,因为运营库存**不含** `base_url`,编辑表单预填不了就会
把它清空。这里三个读模型都是完整的:池与绑定的读模型**等于**输入模型,节点的读模型含
更新所需的全部字段(唯一缺的 `proxy_endpoint` 是只写的,GET 同样不会返回)。
所以逐行再拉一次只是多一个往返,拿不到任何新东西。它们属于批 D3 的详情抽屉,不属于这里。

### 24.8 前端校验地址,但从不拨号

`validateProxyEndpoint` 逐条镜像 `UpstreamProxy::try_socks5`:scheme 必须 socks5、
不接受用户名/密码、主机与端口都必须写明、不接受路径/查询串/片段。它**检查一个 operator
打进来的字符串**,既不拼装地址也不打开任何连接 —— 网关始终是唯一会去连那个代理的东西。
价值是把一个不透明的 400 换成一句能照着改的话。
