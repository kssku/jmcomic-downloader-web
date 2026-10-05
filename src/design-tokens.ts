// 设计令牌 —— 全站颜色的唯一来源。
//
// 改主色只改这里的 palette.primary.*，以下三处自动跟随：
//   1. uno.config.ts 的 theme.colors.primary  -> text-primary / bg-primary 等 utility
//   2. src/theme.ts 的 GlobalThemeOverrides   -> naive-ui 组件内部变量
//   3. src/main.ts 注入的 :root CSS 变量       -> 手写 CSS 里的 var(--primary-color)
//
// 任何地方都不应再出现字面量色值。

export const palette = {
  // 交互类：按钮 / 链接 / 焦点环 / 选中态
  primary: {
    DEFAULT: '#f0c898',
    hover: '#f7d8b0',
    pressed: '#d9a878',
    suppl: '#f7d8b0',
  },
  // 装饰类：卡片描边 / 色条 / 徽章边框。
  // 浅色底上要比深色主题深一档才看得清。
  secondary: {
    DEFAULT: '#7aa8e0',
    hover: '#94b8e8',
    pressed: '#5a90d0',
    suppl: '#94b8e8',
  },
  bg: {
    // 淡蓝底。由调试面板「淡蓝底明度」滑块的映射固化而来：
    //   bodyColor(v) = rgb(v, v+8, v+18)
    // 当前 v = 225 → rgb(225, 233, 243) = #e1e9f3。
    // 滑块是「固定 R = v、G/B 相对 R 加固定偏移」，不是等比缩放；
    // 这样无论明度怎么变，蓝调偏移量恒定，不会在提亮时把蓝调冲淡。
    // 要改明度就改 R 并同步 G=R+8、B=R+18，保持同一算法。
    body: '#e1e9f3',
    // card / raised 是淡蓝调半透明玻璃，让 body::before 的背景图透出来。
    // 两者当前同值，但语义不同：
    //   card   = 大容器（左面板 / 右列表 / Authorization / 弹窗）
    //   raised = 内嵌块（任务行 / 关于页信息条）
    // 将来若要重新分化，各改各的值即可，不要为此合并这两个名字。
    //
    // 页面上其实有三层，alpha 必须拉开，否则层与层之间没有边界，
    // 背景图从最外一路透到最内，看起来像「面板消失了」。
    //
    //   最外  页面底      body   实心 #e1e9f3
    //   中间  大容器      panel  ← 应该最虚，让背景图透出来
    //   最内  行卡片      card   ← 应该比容器实，行和行的边界才在
    //
    // RGB 基底固定为 (230,238,248)，与上面 body 的映射无关 ——
    // 玻璃载体始终是淡蓝调白，不随明度滑块变。
    // 2026-10-04 修复：card / raised 的 alpha 曾是 0.00，与消费端的
    //   color-mix(in srgb, var(--bg-raised) calc(var(--row-alpha) * 100%), transparent)
    // 形成两层相乘 —— 0 × 0.44 = 0，任务行与详情面板同时完全透明，
    // 且「任务行 alpha」「详情面板 alpha」两个滑块彻底失效。
    //
    // 现在改为「单层」模型：card / raised 固定为满 alpha 的载体色，
    // 实际透明度只由 row-alpha / detail-alpha 两个独立滑块经 color-mix 决定。
    // 这里保留 alpha=1 的写法，让 color-mix 的百分比成为唯一乘数。
    // 注意 palette.bg.* 是给 design-tokens 自己的 CSS 变量兜底用的，
    // 调试面板在运行时会把 --bg-raised / --bg-card 覆盖成满 alpha 载体色。
    panel: 'rgba(230, 238, 248, 0.18)',
    card: 'rgba(230, 238, 248, 1)',
    // raised 是内嵌块（任务行 / 输入框 / 关于页信息条），语义同 card 但独立命名，
    // 将来若要重新分化（比如输入框更透），各改各的值即可。
    raised: 'rgba(230, 238, 248, 1)',
    // selected 是「行被选中」的档位，比 raised 更实一档，让选中行从列表里跳出来。
    // 它不参与 panel/card/raised 那套层级推导 —— 它表达的是状态，不是层级。
    selected: 'rgba(230, 238, 248, 0.58)',
  },
  text: {
    base: '#2a2a3a',
    // secondary / tertiary 原为 #5a5a70 / #8a8aa0（灰）。灰在玻璃+淡蓝底上
    // 对比度不足，用户反馈看不清。改近黑，secondary 带一点蓝调，
    // 比纯黑铺在淡蓝底上更自然；tertiary 比 secondary 浅一档，仍然够黑。
    secondary: '#1a1a2e',
    tertiary: '#2a2a3e',
  },
  // 语义状态色。任务状态 pill、进度条失败态、日志级别都用这一组。
  // 之前散落在 ProgressesPane 的 text-blue-500 / text-red-500 等 utility 里，
  // 收进 token 后状态色只有一个来源，改主题时不会漏。
  //
  // 全部取低饱和马卡龙档（S 28%~48% / V 79%~88%），不用 Tailwind 默认的
  // #22c55e / #3b82f6 / #f59e0b / #ef4444 —— 那组 S、V 都在 90% 上下，
  // 铺在玻璃 + 淡蓝底（#e1e9f3）上会跳出画面，和整体柔和调性冲突。
  //
  //   success #7ac99a  H=145 S=28% V=79%
  //   info    #7aa8e0  H=210 S=31% V=88%   （与 palette.secondary.DEFAULT 同值，
  //                                        但语义不同：secondary 是装饰描边，
  //                                        info 是「下载中」状态，不合并）
  //   warning #e8743a  H=20  S=75% V=91%   （主色 H=33，拉开 13° 避免混淆）
  //   error   #e83b52  H=352 S=75% V=91%
  //   neutral #3a3ac4  H=240 S=70% V=77%
  state: {
    success: '#45c46c',
    info: '#4d94e8',
    // 警告从 H=38（黄橙）改到 H=20（红橙）—— 主色是 H=33，
    // 两者只差 5°，主按钮和 warning pill 相邻时会混淆。20° 拉开 13°。
    warning: '#e8743a',
    error: '#e83b52',
    neutral: '#3a3ac4',
    // 取消态。原先与 `neutral` 同色，导致「排队中」和「已取消」在列表里
    // 无法用颜色区分（两者都是蓝色）。改用低饱和冷灰紫（H=250 S=15% L=55%），
    // 与 neutral 的 H=240 拉开色相距离，但饱和度低到不会与 error/info 抢注意力。
    cancelled: '#8a86a6',
  },
}

// 玻璃面板的框线。三个维度分开，方便调试面板各自独立控制：
//   color  框线色（默认 #b34747 = H=0° S=60% V=70% 的红）
//   alpha  可见度（1.0 = 完全不透明）
//   width  粗细（px）
// 消费端用 color-mix 把 alpha 乘进去，所以改 alpha 不需要重算 hex。
//
// 历史：旧实现只有一个「边框色相」滑块，而 hueToHex() 把 S/V 写死在
// S=13.8% / V=91%，于是 H=360°（≡0° 红）算出来是 #e8c8c8 淡灰粉，
// 「红色边框」根本红不起来。现在调试面板的边框组有 5 个滑块
// （H / S / V / alpha / width），由 hsvBorderHex(h, s, v) 三轴计算，
// 默认 H=0 / S=60% / V=70% → #b34747。想回到淡灰粉就把 S 降到 13.8%。
export const border = {
  color: '#b34747',
  alpha: 1.0,
  width: 3,
}

// 玻璃 backdrop-filter 的饱和度倍数。
export const GLASS_SATURATE = 1.2

export const radii = {
  md: '16px',
  sm: '10px',
}

// 全局背景图叠加强度与玻璃模糊半径。
// 这两个值原本分别写死在 main.ts 和 5 个容器的内联 style 里，
// 现在提到这里作为默认值，并由 :root 变量下发。
export const WALLPAPER_OPACITY = 0.6
export const GLASS_BLUR = 0

export const fontFamily =
  '-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif'

// 展开成 CSS 自定义属性，供 src/main.ts 注入 :root。
export const cssVars: Record<string, string> = {
  '--primary-color': palette.primary.DEFAULT,
  '--primary-color-hover': palette.primary.hover,
  '--primary-color-pressed': palette.primary.pressed,
  '--primary-color-suppl': palette.primary.suppl,
  '--secondary-color': palette.secondary.DEFAULT,
  '--secondary-color-hover': palette.secondary.hover,
  '--secondary-color-pressed': palette.secondary.pressed,
  '--secondary-color-suppl': palette.secondary.suppl,
  '--bg-body': palette.bg.body,
  // panel = 大容器（左面板 / 右列表 / Authorization），比 card 虚，让背景图透出来。
  // card / raised = 行卡片与内嵌块，比容器实，保住行与行的边界。
  '--bg-panel': palette.bg.panel,
  '--bg-card': palette.bg.card,
  '--bg-raised': palette.bg.raised,
  // 选中行档位，比 raised 实，表达状态而非层级。
  '--bg-selected': palette.bg.selected,
  // 框线拆成三个独立维度：色 / 可见度 / 粗细。
  // 消费端写 color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent)，
  // 改 alpha 不必重算 hex，改色相也不必动 alpha。
  '--border-color': border.color,
  '--border-alpha': String(border.alpha),
  '--border-width': `${border.width}px`,
  '--text-base': palette.text.base,
  '--text-secondary': palette.text.secondary,
  '--text-tertiary': palette.text.tertiary,
  // 状态色。消费端用 color-mix 做 14% 底的 pill 背景。
  '--state-success': palette.state.success,
  '--state-info': palette.state.info,
  '--state-warning': palette.state.warning,
  '--state-error': palette.state.error,
  '--state-neutral': palette.state.neutral,
  '--state-cancelled': palette.state.cancelled,
  // —— 色块 alpha 变量 ——
  // 调试面板「色块」分组直接改这几个值，消费端的 color-mix 百分比改成读变量，
  // 这样调参不需要改组件代码。
  //   pill-alpha          状态 pill 背景的不透明度（原写死 14%）
  //   sidebar-active-alpha 侧栏激活项背景不透明度（原写死 10%）
  //   sidebar-hover-alpha  侧栏 hover 背景不透明度（bg-raised 的档位）
  //   row-alpha           任务行卡片背景不透明度（bg-raised 的档位）
  //   detail-alpha        详情面板背景不透明度（bg-card 的档位）
  '--pill-alpha': '0.14',
  '--sidebar-active-alpha': '0.10',
  '--sidebar-hover-alpha': '0.44',
  '--row-alpha': '0.44',
  '--detail-alpha': '0.44',
  // 侧栏激活项的色相载体。默认等于主色；调试面板调色相时只改这个，
  // 不动 --primary-color（主色还被按钮、竖条等复用）。
  // 已被调参固化为 H=33° 的橙黄（从 #f0c898 同 S/V 推得）。
  '--sidebar-active-color': '#f0c898',
  '--radius-md': radii.md,
  '--radius-sm': radii.sm,
  // 调试面板可实时覆盖的变量（见 src/components/DebugPanel.vue）。
  // 背景图层与玻璃模糊半径原本写死在 main.ts / 各容器的内联 style 里，
  // 提到 :root 变量后，调试面板只需 setProperty 即可全局生效。
  '--wallpaper-opacity': String(WALLPAPER_OPACITY),
  '--glass-blur': `${GLASS_BLUR}px`,
  '--glass-saturate': String(GLASS_SATURATE),
}

export function cssVarsToCss(selector = ':root'): string {
  const body = Object.entries(cssVars)
    .map(([k, v]) => `  ${k}: ${v};`)
    .join('\n')
  return `${selector} {\n${body}\n}\n`
}