// 调参面板主题 —— 「值 → CSS 变量」的映射。
//
// 独立成模块的原因：主题值现在存后端（config.theme），必须在**启动时**就应用，
// 而 DebugPanel 是懒加载的（AppContent 里 `v-else` 渲染，用户不打开「调参」
// 就完全不 mount）。所以映射逻辑不能留在 DebugPanel 里，否则默认主题永远不生效。
//
// 写的是 `document.documentElement.style.setProperty`（inline style），
// 不是替换 main.ts 注入的那段 <style> —— inline 优先级更高，能覆盖 design-tokens
// 的静态值。DebugPanel 原本就是这个机制，这里只是把它抽出来。
//
// ⚠️ 默认值三处必须同步：
//      src/design-tokens.ts
//      src-server/src/config.rs 的 `impl Default for ThemeConfig`
//      src/components/DebugPanel.vue 的 DEFAULT_* 常量
//    改任一处时，另两处一起改。

import type { ThemeConfig } from './bindings.ts'
import {
  palette,
  border as borderTokens,
  WALLPAPER_OPACITY,
  GLASS_BLUR,
  GLASS_SATURATE,
} from './design-tokens'

// ────────────────────────────────────────────────────────────
// 常量（与 DebugPanel 的 DEFAULT_* 同源）
// ────────────────────────────────────────────────────────────

/** 玻璃载体的 RGB 基底。载体始终是淡蓝调白，与 body 明度映射无关。 */
const GLASS_RGB = '230, 238, 248'
/** --bg-raised / --bg-card 的满 alpha 载体色（透明度交给各自的 alpha 旋钮）。 */
const RAISED_RGB = `rgb(${GLASS_RGB})`

/** 状态色的 S/V，从 palette.state 逐个解析。hue 只动 H，S/V 保持 token 原值。 */
function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '')
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ]
}

function svOf(hex: string): { s: number; v: number } {
  const [r, g, b] = hexToRgb(hex).map((n) => n / 255)
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const v = max
  const s = max === 0 ? 0 : (max - min) / max
  return { s, v }
}

const SVC = {
  success: svOf(palette.state.success),
  info: svOf(palette.state.info),
  warning: svOf(palette.state.warning),
  error: svOf(palette.state.error),
  neutral: svOf(palette.state.neutral),
}

/** 主色的 RGB 基底，从 design-tokens 的 #f0a8c8 解析。面板只调 alpha。 */
const [PR, PG, PB] = hexToRgb(palette.primary.DEFAULT)

// ────────────────────────────────────────────────────────────
// 颜色换算
// ────────────────────────────────────────────────────────────

/** HSV → hex。h∈[0,360)，s/v∈[0,1]。 */
function hsvToHex(h: number, s: number, v: number): string {
  const c = v * s
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1))
  const m = v - c
  let r = 0,
    g = 0,
    b = 0
  if (h < 60) [r, g, b] = [c, x, 0]
  else if (h < 120) [r, g, b] = [x, c, 0]
  else if (h < 180) [r, g, b] = [0, c, x]
  else if (h < 240) [r, g, b] = [0, x, c]
  else if (h < 300) [r, g, b] = [x, 0, c]
  else [r, g, b] = [c, 0, x]
  const to = (n: number) => Math.round((n + m) * 255).toString(16).padStart(2, '0')
  return `#${to(r)}${to(g)}${to(b)}`
}

/** 状态 pill 的 hex：hue 可变，S/V 取 token 原值。 */
function pillHex(key: 'success' | 'info' | 'warning' | 'error' | 'neutral', hue: number): string {
  return hsvToHex(hue, SVC[key].s, SVC[key].v)
}

/** 淡蓝底：保持蓝调，只动明度基底。body = (v, v+8, v+18)。 */
function bodyColor(v: number): string {
  return `rgb(${v}, ${v + 8}, ${v + 18})`
}

// ────────────────────────────────────────────────────────────
// 默认值
// ────────────────────────────────────────────────────────────

/**
 * design-tokens 的默认主题值。
 *
 * 与后端 `ThemeConfig::default()` 逐字段对齐；启动时 getConfig() 拿不到
 * （网络失败 / 旧后端）时的兜底也用它。
 */
export function readThemeDefaults(): ThemeConfig {
  return {
    panelAlpha: 0.18,
    wallpaper: WALLPAPER_OPACITY,
    blur: GLASS_BLUR,
    bgBase: 225,
    primaryAlpha: 1.0,
    borderHue: 0,
    borderSat: 0.6,
    borderVal: 0.7,
    borderAlpha: borderTokens.alpha,
    borderWidth: borderTokens.width,
    saturate: GLASS_SATURATE,
    pillHueSuccess: 144,
    pillHueInfo: 213,
    // 警告从 38°（黄橙）改到 20°（红橙）—— 主色是 33°，两者只差 5°，
    // 按钮和 warning pill 相邻时会混淆。20° 拉开 13° 的间距。
    pillHueWarning: 20,
    pillHueError: 352,
    pillHueNeutral: 240,
    pillAlpha: 0.14,
    sidebarActiveHue: 33,
    sidebarActiveAlpha: 0.1,
    sidebarHoverAlpha: 0.44,
    rowAlpha: 0.44,
    detailAlpha: 0.44,
  }
}

// ────────────────────────────────────────────────────────────
// 应用
// ────────────────────────────────────────────────────────────

/** 把一份 ThemeConfig 全量写进 :root 的 inline style。 */
export function applyTheme(theme: ThemeConfig): void {
  const root = document.documentElement.style

  // 玻璃 / 容器
  root.setProperty('--bg-panel', `rgba(${GLASS_RGB}, ${theme.panelAlpha})`)
  root.setProperty('--wallpaper-opacity', String(theme.wallpaper))
  root.setProperty('--glass-blur', `${theme.blur}px`)
  root.setProperty('--glass-saturate', String(theme.saturate))
  root.setProperty('--bg-body', bodyColor(theme.bgBase))
  root.setProperty('--primary-color', `rgba(${PR}, ${PG}, ${PB}, ${theme.primaryAlpha})`)

  // 边框
  root.setProperty(
    '--border-color',
    hsvToHex(theme.borderHue, theme.borderSat, theme.borderVal),
  )
  root.setProperty('--border-alpha', String(theme.borderAlpha))
  root.setProperty('--border-width', `${theme.borderWidth}px`)

  // 状态色
  root.setProperty('--state-success', pillHex('success', theme.pillHueSuccess))
  root.setProperty('--state-info', pillHex('info', theme.pillHueInfo))
  root.setProperty('--state-warning', pillHex('warning', theme.pillHueWarning))
  root.setProperty('--state-error', pillHex('error', theme.pillHueError))
  root.setProperty('--state-neutral', pillHex('neutral', theme.pillHueNeutral))
  root.setProperty('--pill-alpha', String(theme.pillAlpha))

  // 侧栏 / 行 / 详情
  //
  // 侧栏激活色只改色相，S/V 取主色 token。
  root.setProperty(
    '--sidebar-active-color',
    hsvToHex(theme.sidebarActiveHue, svOf(palette.primary.DEFAULT).s, svOf(palette.primary.DEFAULT).v),
  )
  root.setProperty('--sidebar-active-alpha', String(theme.sidebarActiveAlpha))
  root.setProperty('--sidebar-hover-alpha', String(theme.sidebarHoverAlpha))

  // --bg-raised / --bg-card 固定为满 alpha 的载体色，透明度只由各自的 alpha
  // 旋钮经 color-mix 决定 —— 一层乘法，行 alpha 和详情 alpha 互不干扰。
  root.setProperty('--bg-raised', RAISED_RGB)
  root.setProperty('--bg-card', RAISED_RGB)
  root.setProperty('--row-alpha', String(theme.rowAlpha))
  root.setProperty('--detail-alpha', String(theme.detailAlpha))
}