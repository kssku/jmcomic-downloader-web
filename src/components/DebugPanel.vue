<!--
  调参面板 —— 常驻（生产 build 也渲染），挂在主区「调参」pane 里。

  设计要点：
  - 不走 Vue 响应式。滑块的 oninput 直接 setProperty 到 :root，避免整个 app 重渲染。
  - 22 个滑块按语义分 4 组（玻璃 / 边框 / 背景 / 色块），组标题可折叠。
  - 面板本身用玻璃效果（--bg-panel + backdrop-filter），和其他 pane 保持一致。
  - 值存 localStorage：参数 jm-devtools-values，色块 jm-devtools-colors，
    分组展开状态 jm-devtools-groups。

  调好后点「导出 JSON」，把文本粘回对话即可固化。
-->
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { PhCaretDown, PhCaretRight } from '@phosphor-icons/vue'
import {
  palette,
  border as borderTokens,
  WALLPAPER_OPACITY,
  GLASS_BLUR,
  GLASS_SATURATE,
} from '../design-tokens'

// 面板是主区「调参」pane 的内容（生产 build 也渲染），不再是右下角浮层。
// 值分两个 key 存 localStorage：容器/玻璃参数、语义色块各一组。

// 默认值：与 design-tokens.ts 保持同源，避免这里再写一份字面量。
// 玻璃 alpha 与淡蓝底明度没有对应的 token 常量（alpha 内嵌在 rgba 字符串里、
// body 是 hex），所以这两个只能写在这里；改动时要和 design-tokens.ts 一起改。
const DEFAULT_PANEL_ALPHA = 0.18
const DEFAULT_WALLPAPER = WALLPAPER_OPACITY
const DEFAULT_BLUR = GLASS_BLUR
const DEFAULT_BG_BASE = 225
const DEFAULT_PRIMARY_ALPHA = 1.0
// 边框默认改成真正的红：H=0 / S=60% / V=70% → #b34747。
// 旧的 360°/S13.8%/V91% 组合渲染出来是 #e8c8c8 淡灰粉，用户不接受。
const DEFAULT_BORDER_HUE = 0
const DEFAULT_BORDER_SAT = 0.6
const DEFAULT_BORDER_VAL = 0.7
const DEFAULT_BORDER_ALPHA = borderTokens.alpha
const DEFAULT_BORDER_WIDTH = borderTokens.width
const DEFAULT_SATURATE = GLASS_SATURATE

// —— 色块分组默认值 ——
//
// 这一组覆盖「文字色 + 方块背景」的 5 类组合。每类两个自由度：
//   文字色相 H（0-360，保持当前 S/V 不变）
//   背景 alpha（0-1.0，即 color-mix 里的百分比）
//
// 状态 pill 的 5 个状态色各自独立 —— 用户要能分别调「失败是红的还是橙的」，
// 所以不是共用一套控件。
//
// 文字色的 S/V 从 design-tokens 的 palette.state.* 解析出来，改 token 时这里
// 自动跟随，不写第二份字面量。

// 玻璃载体的 RGB 基底。与 body 的明度映射无关 —— 载体始终是淡蓝调白。
const GLASS_RGB = '230, 238, 248'

// 边框色：H / S / V 三个自由度都可调。
//
// 早先只有色相一个滑块，S/V 写死成 token border.color (#c8d4e8) 的 13.8% / 91%。
// 后果是「边框色相 360°」渲染出来是 #e8c8c8 淡灰粉而不是红 —— 低饱和把红压灰了。
// 现在把 S/V 也开放成滑块，默认 H=0 / S=60% / V=70% → #b34747 正红。
// 想调回原先的淡灰粉，把 S 拉到 14%、V 拉到 91% 即可。
function hsvBorderHex(h: number, s: number, v: number): string {
  const c = v * s
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1))
  const m = v - c
  let r = 0, g = 0, b = 0
  if (h < 60) [r, g, b] = [c, x, 0]
  else if (h < 120) [r, g, b] = [x, c, 0]
  else if (h < 180) [r, g, b] = [0, c, x]
  else if (h < 240) [r, g, b] = [0, x, c]
  else if (h < 300) [r, g, b] = [x, 0, c]
  else [r, g, b] = [c, 0, x]
  const to = (n: number) =>
    Math.round((n + m) * 255).toString(16).padStart(2, '0')
  return `#${to(r)}${to(g)}${to(b)}`
}

// 主色的 RGB 基底，从 design-tokens 的 #f0a8c8 解析出来。
// 面板只调 alpha，色相不动。
function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '')
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ]
}
const [PR, PG, PB] = hexToRgb(palette.primary.DEFAULT)

// 淡蓝底：保持蓝调，只动明度基底。
// body = (v, v+8, v+18) —— v=230 时正好是 #e6eef8，与 design-tokens 一致。
function bodyColor(v: number): string {
  return `rgb(${v}, ${v + 8}, ${v + 18})`
}

// 状态色的 S/V，从 palette.state 逐个解析。
// hueToHex 只动 H，S/V 保持 token 原值 —— 这样默认色相能精确往返 token。
function svOf(hex: string): { s: number; v: number } {
  const [r, g, b] = hexToRgb(hex).map((n) => n / 255)
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const v = max
  const s = max === 0 ? 0 : (max - min) / max
  return { s, v }
}

// 通用 HSV->hex。hueToHex 是给边框色用的（固定 S/V），这个给色块用（每个色自带 S/V）。
function hsvToHex(h: number, s: number, v: number): string {
  const c = v * s
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1))
  const m = v - c
  let r = 0, g = 0, b = 0
  if (h < 60) [r, g, b] = [c, x, 0]
  else if (h < 120) [r, g, b] = [x, c, 0]
  else if (h < 180) [r, g, b] = [0, c, x]
  else if (h < 240) [r, g, b] = [0, x, c]
  else if (h < 300) [r, g, b] = [x, 0, c]
  else [r, g, b] = [c, 0, x]
  const to = (n: number) => Math.round((n + m) * 255).toString(16).padStart(2, '0')
  return `#${to(r)}${to(g)}${to(b)}`
}

// hex -> 色相角（0-360），保持原 S/V。用于把 token 色换算成滑块初值。

const SVC = {
  success: svOf(palette.state.success),
  info: svOf(palette.state.info),
  warning: svOf(palette.state.warning),
  error: svOf(palette.state.error),
  neutral: svOf(palette.state.neutral),
}

// 五类色块的默认值。pill 系列 alpha 默认 0.14（现有 color-mix 的 14%）。
// 用户固化值：色相直接写目标度数，不从 palette 推导 —— palette.state.* 的
// hex 是「按目标 H + 各自 S/V 生成的马卡龙色」，反推 H 会有 1° 的取整误差
// （success 目标 144 实测 138、warning 目标 38 实测 42）。
// 滑块默认值必须以「用户拖到的度数」为准，否则一重载就跳。
const DEFAULT_HUE_SUCCESS = 144
const DEFAULT_HUE_INFO = 213
// 警告从 38°（黄橙）改到 20°（红橙）—— 主色是 33°，两者只差 5°，
// 按钮和 warning pill 相邻时会混淆。20° 拉开 13° 的间距。
const DEFAULT_HUE_WARNING = 20
const DEFAULT_HUE_ERROR = 352
const DEFAULT_HUE_NEUTRAL = 240
const DEFAULT_PILL_ALPHA = 0.14
const DEFAULT_SIDEBAR_ACTIVE_ALPHA = 0.10
const DEFAULT_SIDEBAR_ACTIVE_HUE = 33
const DEFAULT_SIDEBAR_HOVER_ALPHA = 0.44
const DEFAULT_ROW_ALPHA = 0.44
const DEFAULT_DETAIL_ALPHA = 0.44

const pillHueSuccess = ref(DEFAULT_HUE_SUCCESS)
const pillHueInfo = ref(DEFAULT_HUE_INFO)
const pillHueWarning = ref(DEFAULT_HUE_WARNING)
const pillHueError = ref(DEFAULT_HUE_ERROR)
const pillHueNeutral = ref(DEFAULT_HUE_NEUTRAL)
const pillAlpha = ref(DEFAULT_PILL_ALPHA)
const sidebarActiveHue = ref(DEFAULT_SIDEBAR_ACTIVE_HUE)
const sidebarActiveAlpha = ref(DEFAULT_SIDEBAR_ACTIVE_ALPHA)
const sidebarHoverAlpha = ref(DEFAULT_SIDEBAR_HOVER_ALPHA)
const rowAlpha = ref(DEFAULT_ROW_ALPHA)
const detailAlpha = ref(DEFAULT_DETAIL_ALPHA)

const panelAlpha = ref(DEFAULT_PANEL_ALPHA)
const wallpaper = ref(DEFAULT_WALLPAPER)
const blur = ref(DEFAULT_BLUR)
const bgBase = ref(DEFAULT_BG_BASE)
const primaryAlpha = ref(DEFAULT_PRIMARY_ALPHA)
const borderHue = ref(DEFAULT_BORDER_HUE)
const borderSat = ref(DEFAULT_BORDER_SAT)
const borderVal = ref(DEFAULT_BORDER_VAL)
const borderAlpha = ref(DEFAULT_BORDER_ALPHA)
const borderWidth = ref(DEFAULT_BORDER_WIDTH)
const saturate = ref(DEFAULT_SATURATE)

const root = () => document.documentElement.style

// 每个 handler 直接吃 input 事件里的新值，不读 ref。
//
// 为什么不用 v-model：v-model 和显式 @input 会往同一个元素上挂两个 onInput，
// Vue 把 v-model 的更新挂在后面执行，于是 apply*() 读到的是上一帧的 ref ——
// 滑块拖一格，CSS 变量慢一拍，label 也跟着慢一拍。
// 直接传值可以从根上避免这个顺序问题。
function setPanel(a: number) {
  root().setProperty('--bg-panel', `rgba(${GLASS_RGB}, ${a})`)
}
// —— 色块 setter ——
//
// 写的是「消费端已经在读的那几个变量」，不改消费端的表达式：
//   pill      -> --state-* 本身（写 hex，消费端 color-mix 14% 用的是 --pill-alpha）
//   pill 底   -> --pill-alpha
//   侧栏激活  -> --primary-color 的色相（改写 --sidebar-active-color）+ --sidebar-active-alpha
//   侧栏 hover-> --sidebar-hover-alpha（消费端用 rgba 形式）
//   行卡片    -> 直接改 --bg-raised 的 alpha
//   详情面板  -> 直接改 --bg-card 的 alpha
function pillHex(key: 'success' | 'info' | 'warning' | 'error' | 'neutral', hue: number): string {
  return hsvToHex(hue, SVC[key].s, SVC[key].v)
}
function setPillHue(key: 'success' | 'info' | 'warning' | 'error' | 'neutral', hue: number) {
  root().setProperty(`--state-${key}`, pillHex(key, hue))
}
function setPillAlpha(a: number) {
  root().setProperty('--pill-alpha', String(a))
}
function setSidebarActiveHue(h: number) {
  const hex = hsvToHex(h, svOf(palette.primary.DEFAULT).s, svOf(palette.primary.DEFAULT).v)
  root().setProperty('--sidebar-active-color', hex)
}
function setSidebarActiveAlpha(a: number) {
  root().setProperty('--sidebar-active-alpha', String(a))
}
function setSidebarHoverAlpha(a: number) {
  root().setProperty('--sidebar-hover-alpha', String(a))
}
// 修复 1：任务行 alpha / 详情面板 alpha 各自独立，不再乘「卡片 alpha」。
//
// 旧实现里 --bg-raised 和 --bg-card 的 alpha 都等于「卡片 alpha」，消费端再
// 分别乘 --row-alpha / --detail-alpha。用户把「卡片 alpha」拖到 0 之后两个乘法
// 都是 0，任务行和详情面板一起消失，另外两个滑块彻底失效。
//
// 现在 --bg-raised / --bg-card 固定为满 alpha 的载体色，只由各自的 alpha 旋钮
// 经 color-mix 决定实际透明度 —— 一层乘法，两个滑块互不干扰。
const RAISED_RGB = `rgb(${GLASS_RGB})`
function setRowAlpha(a: number) {
  root().setProperty('--bg-raised', RAISED_RGB)
  root().setProperty('--row-alpha', String(a))
}
function setDetailAlpha(a: number) {
  root().setProperty('--bg-card', RAISED_RGB)
  root().setProperty('--detail-alpha', String(a))
}

function setBorder() {
  root().setProperty(
    '--border-color',
    hsvBorderHex(borderHue.value, borderSat.value, borderVal.value),
  )
  root().setProperty('--border-alpha', String(borderAlpha.value))
  root().setProperty('--border-width', `${borderWidth.value}px`)
}
function setSaturate() {
  root().setProperty('--glass-saturate', String(saturate.value))
}

// —— 持久化 ——
//
// 值存成普通对象，读回来时逐字段校验类型，避免 localStorage 里
// 手工改坏的字符串把 ref 变成 NaN 导致整页样式归零。
const LS_VALUES = 'jm-devtools-values'
// 「色块」分组单独一个 key。和 LS_VALUES 分开的理由：色块是「语义色」，
// 和容器层级/玻璃参数不是一类东西，用户可能只想重置其中一组。
const LS_COLORS = 'jm-devtools-colors'

function loadValues() {
  try {
    const raw = localStorage.getItem(LS_VALUES)
    if (!raw) return
    const v = JSON.parse(raw) as Record<string, unknown>
    const num = (x: unknown, fallback: number) =>
      typeof x === 'number' && Number.isFinite(x) ? x : fallback
    panelAlpha.value = num(v.panelAlpha, DEFAULT_PANEL_ALPHA)
    wallpaper.value = num(v.wallpaper, DEFAULT_WALLPAPER)
    blur.value = num(v.blur, DEFAULT_BLUR)
    bgBase.value = num(v.bgBase, DEFAULT_BG_BASE)
    primaryAlpha.value = num(v.primaryAlpha, DEFAULT_PRIMARY_ALPHA)
    borderHue.value = num(v.borderHue, DEFAULT_BORDER_HUE)
    borderAlpha.value = num(v.borderAlpha, DEFAULT_BORDER_ALPHA)
    borderWidth.value = num(v.borderWidth, DEFAULT_BORDER_WIDTH)
    saturate.value = num(v.saturate, DEFAULT_SATURATE)
  } catch {
    // 存档损坏就当没存过，用默认值。
  }
  // 色块是独立 key，单独读 —— 一个 key 坏掉不该拖垮另一组。
  try {
    const raw = localStorage.getItem(LS_COLORS)
    if (raw) {
      const v = JSON.parse(raw) as Record<string, unknown>
      const num = (x: unknown, fallback: number) =>
        typeof x === 'number' && Number.isFinite(x) ? x : fallback
      pillHueSuccess.value = num(v.pillHueSuccess, DEFAULT_HUE_SUCCESS)
      pillHueInfo.value = num(v.pillHueInfo, DEFAULT_HUE_INFO)
      pillHueWarning.value = num(v.pillHueWarning, DEFAULT_HUE_WARNING)
      pillHueError.value = num(v.pillHueError, DEFAULT_HUE_ERROR)
      pillHueNeutral.value = num(v.pillHueNeutral, DEFAULT_HUE_NEUTRAL)
      pillAlpha.value = num(v.pillAlpha, DEFAULT_PILL_ALPHA)
      sidebarActiveHue.value = num(v.sidebarActiveHue, DEFAULT_SIDEBAR_ACTIVE_HUE)
      sidebarActiveAlpha.value = num(v.sidebarActiveAlpha, DEFAULT_SIDEBAR_ACTIVE_ALPHA)
      sidebarHoverAlpha.value = num(v.sidebarHoverAlpha, DEFAULT_SIDEBAR_HOVER_ALPHA)
      rowAlpha.value = num(v.rowAlpha, DEFAULT_ROW_ALPHA)
      detailAlpha.value = num(v.detailAlpha, DEFAULT_DETAIL_ALPHA)
    }
  } catch {
    // 同上。
  }
}

function saveValues() {
  try {
    localStorage.setItem(
      LS_VALUES,
      JSON.stringify({
        panelAlpha: panelAlpha.value,
        wallpaper: wallpaper.value,
        blur: blur.value,
        bgBase: bgBase.value,
        primaryAlpha: primaryAlpha.value,
        borderHue: borderHue.value,
        borderAlpha: borderAlpha.value,
        borderWidth: borderWidth.value,
        saturate: saturate.value,
      }),
    )
    // 色块单独落盘。
    localStorage.setItem(LS_COLORS, JSON.stringify(colorValues()))
  } catch {
    // localStorage 不可写（隐私模式 / 配额满）时静默降级，不影响调参本身。
  }
}

// 色块值单独取一份，供 LS_COLORS 使用。
function colorValues() {
  return {
    pillHueSuccess: pillHueSuccess.value,
    pillHueInfo: pillHueInfo.value,
    pillHueWarning: pillHueWarning.value,
    pillHueError: pillHueError.value,
    pillHueNeutral: pillHueNeutral.value,
    pillAlpha: pillAlpha.value,
    sidebarActiveHue: sidebarActiveHue.value,
    sidebarActiveAlpha: sidebarActiveAlpha.value,
    sidebarHoverAlpha: sidebarHoverAlpha.value,
    rowAlpha: rowAlpha.value,
    detailAlpha: detailAlpha.value,
  }
}

// 挂载：先读存档值，再统一写进 :root。
// 顺序不能反 —— applyAll 读的是 ref，ref 必须先被存档覆盖。
onMounted(() => {
  loadValues()
  loadGroups()
  applyAll()
})

function reset() {
  panelAlpha.value = DEFAULT_PANEL_ALPHA
  wallpaper.value = DEFAULT_WALLPAPER
  blur.value = DEFAULT_BLUR
  bgBase.value = DEFAULT_BG_BASE
  primaryAlpha.value = DEFAULT_PRIMARY_ALPHA
  borderHue.value = DEFAULT_BORDER_HUE
  borderSat.value = DEFAULT_BORDER_SAT
  borderVal.value = DEFAULT_BORDER_VAL
  borderAlpha.value = DEFAULT_BORDER_ALPHA
  borderWidth.value = DEFAULT_BORDER_WIDTH
  saturate.value = DEFAULT_SATURATE
  pillHueSuccess.value = DEFAULT_HUE_SUCCESS
  pillHueInfo.value = DEFAULT_HUE_INFO
  pillHueWarning.value = DEFAULT_HUE_WARNING
  pillHueError.value = DEFAULT_HUE_ERROR
  pillHueNeutral.value = DEFAULT_HUE_NEUTRAL
  pillAlpha.value = DEFAULT_PILL_ALPHA
  sidebarActiveHue.value = DEFAULT_SIDEBAR_ACTIVE_HUE
  sidebarActiveAlpha.value = DEFAULT_SIDEBAR_ACTIVE_ALPHA
  sidebarHoverAlpha.value = DEFAULT_SIDEBAR_HOVER_ALPHA
  rowAlpha.value = DEFAULT_ROW_ALPHA
  detailAlpha.value = DEFAULT_DETAIL_ALPHA
  applyAll()
  try {
    localStorage.removeItem(LS_VALUES)
    localStorage.removeItem(LS_COLORS)
  } catch {
    // 静默降级。
  }
}

// applyAll：把当前 ref 全量写进 :root。只给 onMounted / reset 用 ——
// 拖滑块走的是 onSlider -> it.set()，不经过这里。
function applyAll() {
  setPanel(panelAlpha.value)
  setRowAlpha(rowAlpha.value)
  setDetailAlpha(detailAlpha.value)
  setBorder()
  setSaturate()
  root().setProperty('--wallpaper-opacity', String(wallpaper.value))
  root().setProperty('--glass-blur', `${blur.value}px`)
  root().setProperty('--bg-body', bodyColor(bgBase.value))
  root().setProperty(
    '--primary-color',
    `rgba(${PR}, ${PG}, ${PB}, ${primaryAlpha.value})`,
  )
  setPillHue('success', pillHueSuccess.value)
  setPillHue('info', pillHueInfo.value)
  setPillHue('warning', pillHueWarning.value)
  setPillHue('error', pillHueError.value)
  setPillHue('neutral', pillHueNeutral.value)
  setPillAlpha(pillAlpha.value)
  setSidebarActiveHue(sidebarActiveHue.value)
  setSidebarActiveAlpha(sidebarActiveAlpha.value)
  setSidebarHoverAlpha(sidebarHoverAlpha.value)
  setRowAlpha(rowAlpha.value)
  setDetailAlpha(detailAlpha.value)
}

// 每次改动后落盘。放在 applyAll 之后调用，保证存的是已生效的值。
function persist() {
  saveValues()
}

// —— 分组折叠 ——
//
// 21 个滑块平铺是一堵墙。按语义分 4 组，组标题可点开/收起，
// 展开状态存 localStorage（jm-devtools-groups）。
// 默认只展开「玻璃」，让用户一进来就看到最常调的那组。
const LS_GROUPS = 'jm-devtools-groups'

// 一个滑块条目的声明：绑哪个 CSS 变量、默认值从哪个 ref 读、写到哪个 ref、
// 以及调哪个 setter。valueOf/onSlider 靠它做统一分发，
// 模板里就不用为 21 个滑块各写一段 :value + @input。
type SliderDef = {
  var: string
  label: string
  min: number
  max: number
  step: number
  unit: string
  // 当前值（读 ref）
  get: () => number
  // 新值（写 ref + CSS 变量 + 落盘）
  set: (v: number) => void
}

const GROUPS: { id: string; title: string; items: SliderDef[] }[] = [
  {
    id: 'glass',
    title: '玻璃',
    items: [
      { var: '--bg-panel', label: '容器 alpha', min: 0, max: 0.6, step: 0.02, unit: '',
        get: () => panelAlpha.value, set: (v) => { panelAlpha.value = v; setPanel(v); persist() } },
      { var: '--glass-blur', label: '模糊半径', min: 0, max: 24, step: 2, unit: 'px',
        get: () => blur.value, set: (v) => { blur.value = v; root().setProperty('--glass-blur', `${v}px`); persist() } },
      { var: '--glass-saturate', label: '饱和度', min: 0.5, max: 2, step: 0.1, unit: '',
        get: () => saturate.value, set: (v) => { saturate.value = v; setSaturate(); persist() } },
    ],
  },
  {
    id: 'border',
    title: '边框',
    items: [
      { var: '--border-color', label: '边框色相', min: 0, max: 360, step: 1, unit: '°',
        get: () => borderHue.value, set: (v) => { borderHue.value = v; setBorder(); persist() } },
      { var: '--border-color', label: '边框饱和度', min: 0, max: 0.8, step: 0.02, unit: '',
        get: () => borderSat.value, set: (v) => { borderSat.value = v; setBorder(); persist() } },
      { var: '--border-color', label: '边框明度', min: 0, max: 1, step: 0.02, unit: '',
        get: () => borderVal.value, set: (v) => { borderVal.value = v; setBorder(); persist() } },
      { var: '--border-alpha', label: '边框深度', min: 0, max: 1, step: 0.05, unit: '',
        get: () => borderAlpha.value, set: (v) => { borderAlpha.value = v; setBorder(); persist() } },
      { var: '--border-width', label: '边框宽度', min: 0, max: 3, step: 0.5, unit: 'px',
        get: () => borderWidth.value, set: (v) => { borderWidth.value = v; setBorder(); persist() } },
    ],
  },
  {
    id: 'bg',
    title: '背景',
    items: [
      { var: '--wallpaper-opacity', label: '背景图 opacity', min: 0, max: 0.6, step: 0.02, unit: '',
        get: () => wallpaper.value, set: (v) => { wallpaper.value = v; root().setProperty('--wallpaper-opacity', String(v)); persist() } },
      { var: '--bg-body', label: '淡蓝底明度', min: 220, max: 250, step: 5, unit: '',
        get: () => bgBase.value, set: (v) => { bgBase.value = v; root().setProperty('--bg-body', bodyColor(v)); persist() } },
      { var: '--primary-color', label: '主色 alpha', min: 0.5, max: 1, step: 0.05, unit: '',
        get: () => primaryAlpha.value, set: (v) => {
          primaryAlpha.value = v
          root().setProperty('--primary-color', `rgba(${PR}, ${PG}, ${PB}, ${v})`)
          persist()
        } },
    ],
  },
  {
    id: 'colors',
    title: '色块',
    items: [
      { var: '--state-success', label: '状态·成功 色相', min: 0, max: 360, step: 1, unit: '°',
        get: () => pillHueSuccess.value, set: (v) => { pillHueSuccess.value = v; setPillHue('success', v); persist() } },
      { var: '--state-info', label: '状态·信息 色相', min: 0, max: 360, step: 1, unit: '°',
        get: () => pillHueInfo.value, set: (v) => { pillHueInfo.value = v; setPillHue('info', v); persist() } },
      { var: '--state-warning', label: '状态·警告 色相', min: 0, max: 360, step: 1, unit: '°',
        get: () => pillHueWarning.value, set: (v) => { pillHueWarning.value = v; setPillHue('warning', v); persist() } },
      { var: '--state-error', label: '状态·错误 色相', min: 0, max: 360, step: 1, unit: '°',
        get: () => pillHueError.value, set: (v) => { pillHueError.value = v; setPillHue('error', v); persist() } },
      { var: '--state-neutral', label: '状态·中性 色相', min: 0, max: 360, step: 1, unit: '°',
        get: () => pillHueNeutral.value, set: (v) => { pillHueNeutral.value = v; setPillHue('neutral', v); persist() } },
      { var: '--pill-alpha', label: '状态 pill 背景 alpha', min: 0, max: 1, step: 0.02, unit: '',
        get: () => pillAlpha.value, set: (v) => { pillAlpha.value = v; setPillAlpha(v); persist() } },
      { var: '--sidebar-active-color', label: '侧栏激活 色相', min: 0, max: 360, step: 1, unit: '°',
        get: () => sidebarActiveHue.value, set: (v) => { sidebarActiveHue.value = v; setSidebarActiveHue(v); persist() } },
      { var: '--sidebar-active-alpha', label: '侧栏激活 背景 alpha', min: 0, max: 1, step: 0.02, unit: '',
        get: () => sidebarActiveAlpha.value, set: (v) => { sidebarActiveAlpha.value = v; setSidebarActiveAlpha(v); persist() } },
      { var: '--sidebar-hover-alpha', label: '侧栏 hover 背景 alpha', min: 0, max: 1, step: 0.02, unit: '',
        get: () => sidebarHoverAlpha.value, set: (v) => { sidebarHoverAlpha.value = v; setSidebarHoverAlpha(v); persist() } },
      { var: '--row-alpha', label: '任务行 背景 alpha', min: 0, max: 1, step: 0.02, unit: '',
        get: () => rowAlpha.value, set: (v) => { rowAlpha.value = v; setRowAlpha(v); persist() } },
      { var: '--detail-alpha', label: '详情面板 背景 alpha', min: 0, max: 1, step: 0.02, unit: '',
        get: () => detailAlpha.value, set: (v) => { detailAlpha.value = v; setDetailAlpha(v); persist() } },
    ],
  },
]

// 组标题左侧小圆点配色：玻璃=蓝、边框=警告色、背景=主色、色块=成功色。
const GROUP_DOT: Record<string, string> = {
  glass: 'var(--secondary-color)',
  border: 'var(--state-warning)',
  bg: 'var(--primary-color)',
  colors: 'var(--state-success)',
}

// 默认只展开「玻璃」。
const openGroups = ref<string[]>(['glass'])

function loadGroups() {
  try {
    const raw = localStorage.getItem(LS_GROUPS)
    if (!raw) return
    const arr = JSON.parse(raw)
    if (Array.isArray(arr)) {
      openGroups.value = arr.filter((x) => typeof x === 'string' && GROUPS.some((g) => g.id === x))
    }
  } catch {
    // 存档损坏就退回默认展开，不影响面板。
  }
}

function saveGroups() {
  try {
    localStorage.setItem(LS_GROUPS, JSON.stringify(openGroups.value))
  } catch {
    // 静默降级。
  }
}

function toggleGroup(id: string) {
  const i = openGroups.value.indexOf(id)
  if (i >= 0) openGroups.value.splice(i, 1)
  else openGroups.value.push(id)
  saveGroups()
}

function valueOf(it: SliderDef): number {
  return it.get()
}

// 数值显示：alpha 保留两位，其它整数；带单位的加单位。
function displayOf(it: SliderDef): string {
  const v = it.get()
  const n = it.step < 1 ? v.toFixed(2) : String(v)
  return it.unit ? `${n}${it.unit}` : n
}

function onSlider(it: SliderDef, e: Event) {
  const v = Number((e.target as HTMLInputElement).value)
  it.set(v)
}

const copied = ref(false)
function flash() {
  copied.value = true
  setTimeout(() => (copied.value = false), 1500)
}

function currentValues() {
  return {
    panelAlpha: panelAlpha.value,
    wallpaper: wallpaper.value,
    blur: blur.value,
    bgBase: bgBase.value,
    primaryAlpha: primaryAlpha.value,
    borderHue: borderHue.value,
    borderAlpha: borderAlpha.value,
    borderWidth: borderWidth.value,
    saturate: saturate.value,
    // 色块分组
    pillHueSuccess: pillHueSuccess.value,
    pillHueInfo: pillHueInfo.value,
    pillHueWarning: pillHueWarning.value,
    pillHueError: pillHueError.value,
    pillHueNeutral: pillHueNeutral.value,
    pillAlpha: pillAlpha.value,
    sidebarActiveHue: sidebarActiveHue.value,
    sidebarActiveAlpha: sidebarActiveAlpha.value,
    sidebarHoverAlpha: sidebarHoverAlpha.value,
    rowAlpha: rowAlpha.value,
    detailAlpha: detailAlpha.value,
  }
}

// 导出 JSON：给用户粘贴回对话用，所以带缩进、字段名和 token 名对应。
function exportJson() {
  const text = JSON.stringify(currentValues(), null, 2)
  navigator.clipboard.writeText(text).then(flash)
}
</script>


<template>
  <div
    data-debug-panel
    class="h-full flex flex-col rounded-xl box-border text-[12px] leading-tight text-[var(--text-base)]"
    style="
      font-family: monospace;
      background: var(--bg-panel);
      backdrop-filter: blur(var(--glass-blur, 12px)) saturate(var(--glass-saturate, 1.2));
      -webkit-backdrop-filter: blur(var(--glass-blur, 12px)) saturate(var(--glass-saturate, 1.2));
      border: var(--border-width) solid
        color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent);
      padding: 20px;
      border-radius: 10px;
    ">
    <div class="flex items-center justify-between shrink-0 mb-3">
      <span class="font-bold text-[var(--text-base)] text-[13px]">调参面板</span>
      <span class="text-[var(--text-tertiary)]">{{ openGroups.length }}/{{ GROUPS.length }} 组展开</span>
    </div>

    <!-- 分组区：4 组可折叠，组间距 16px + 细分隔线，组内行距 8px -->
    <div class="flex-1 overflow-y-auto debug-groups">

      <section
        v-for="g in GROUPS"
        :key="g.id"
        :data-group="g.id"
        class="debug-group">
        <!-- 组标题：小圆点（组色）+ 展开箭头 + 标题 -->
        <button
          :data-group-toggle="g.id"
          class="w-full flex items-center text-left text-[12px] font-bold text-[var(--text-base)] shrink-0"
          style="margin-bottom: 8px; gap: 6px;"
          @click="toggleGroup(g.id)">
          <PhCaretDown v-if="openGroups.includes(g.id)" :size="11" weight="bold" />
          <PhCaretRight v-else :size="11" weight="bold" />
          <span
            class="inline-block shrink-0"
            :data-group-dot="g.id"
            :style="{
              width: '7px', height: '7px', borderRadius: '9999px',
              background: GROUP_DOT[g.id],
            }"></span>
          <span>{{ g.title }}</span>
          <span class="text-[var(--text-tertiary)] font-normal">（{{ g.items.length }}）</span>
        </button>

        <!-- 组内滑块：单行布局 标签110px / 滑块flex / 数值 pill 48px -->
        <div v-show="openGroups.includes(g.id)" :data-group-body="g.id">
          <div
            v-for="it in g.items"
            :key="it.var + it.label"
            class="flex items-center debug-row">
            <span
              class="shrink-0 text-[var(--text-secondary)]"
              style="width: 110px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;"
              :title="it.label">{{ it.label }}</span>
            <input
              :data-var="it.var"
              :value="valueOf(it)"
              type="range"
              :min="it.min" :max="it.max" :step="it.step"
              class="flex-1 debug-range"
              style="min-width: 0;"
              @input="(e) => onSlider(it, e)" />
            <span
              class="shrink-0 slider-value"
              :data-slider-value="it.var">{{ displayOf(it) }}</span>
          </div>
        </div>
      </section>

    </div>

    <!-- 底部固定按钮行：右对齐，primary + ghost 组合 -->
    <div class="shrink-0 flex justify-end items-center" style="margin-top: 16px; gap: 8px;">
      <n-button data-debug-export type="primary" size="small" @click="exportJson">
        {{ copied ? '已复制 ✓' : '导出 JSON' }}
      </n-button>
      <n-button data-debug-reset size="small" ghost @click="reset">
        恢复默认
      </n-button>
    </div>
  </div>
</template>

<style scoped>
/* 组间距 16px。组标题按钮自带 hover 反馈，不再加分隔线。 */
.debug-groups {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* 组容器：圆角 8px，呼应面板的 10px。 */
.debug-group {
  border-radius: 8px;
}

/* 组标题按钮：重置浏览器默认外观（默认是 0px 圆角 + 2px 黑框 + 灰底）。
   圆角 8px 与组容器一致。 */
.debug-group [data-group-toggle] {
  border: none;
  background: transparent;
  border-radius: 8px;
  padding: 6px 8px;
  width: 100%;
  text-align: left;
  cursor: pointer;
  transition: background 0.15s;
}
.debug-group [data-group-toggle]:hover {
  background: rgba(0, 0, 0, 0.04);
}

/* 组内容容器：圆角 8px。 */
.debug-group [data-group-body] {
  border-radius: 8px;
}

/* 滑块行：高 22px，行距 8px，圆角 6px。
   hover 反馈才有意义 —— 没有背景的话圆角是空的。 */
.debug-row {
  height: 22px;
  margin-bottom: 8px;
  gap: 8px;
  border-radius: 6px;
  padding-left: 4px;
  padding-right: 4px;
}
.debug-row:hover {
  background: rgba(0, 0, 0, 0.03);
}

/* 自定义滑块：覆盖浏览器默认灰色方块。 */
.debug-range {
  -webkit-appearance: none;
  appearance: none;
  height: 4px;
  background: rgba(0, 0, 0, 0.1);
  border-radius: 9999px;
  outline: none;
  cursor: pointer;
}
.debug-range::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 14px;
  height: 14px;
  border-radius: 9999px;
  background: var(--primary-color);
  cursor: pointer;
  border: 2px solid #fff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
}
.debug-range::-moz-range-thumb {
  width: 14px;
  height: 14px;
  border-radius: 9999px;
  background: var(--primary-color);
  cursor: pointer;
  border: 2px solid #fff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
}
.debug-range::-moz-range-track {
  height: 4px;
  background: rgba(0, 0, 0, 0.1);
  border-radius: 9999px;
}

/* 数值胶囊。 */
.slider-value {
  background: rgba(0, 0, 0, 0.06);
  padding: 2px 8px;
  border-radius: 9999px;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  min-width: 48px;
  text-align: center;
}
</style>
