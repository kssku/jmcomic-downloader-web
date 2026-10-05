import type { GlobalThemeOverrides } from 'naive-ui'
import { palette, border, radii, fontFamily } from './design-tokens'

// naive-ui 主题。所有色值来自 src/design-tokens.ts（唯一来源）。
// 改主色/底色请改 design-tokens.ts，不要在这里写字面量。
export const violetThemeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: palette.primary.DEFAULT,
    primaryColorHover: palette.primary.hover,
    primaryColorPressed: palette.primary.pressed,
    primaryColorSuppl: palette.primary.suppl,
    bodyColor: palette.bg.body,
    cardColor: palette.bg.card,
    modalColor: palette.bg.card,
    popoverColor: palette.bg.raised,
    tableColor: palette.bg.card,
    // 输入框背景透明：所有可见的 n-input 都嵌在玻璃容器里
    // （左侧面板 / Authorization 区 / 登录弹窗），自己再画一层 0.4 会叠成约 0.64。
    // 玻璃容器负责那一层半透明，输入框只留边框。
    inputColor: 'transparent',
    borderColor: border.color,
    textColorBase: palette.text.base,
    textColor1: palette.text.base,
    textColor2: palette.text.secondary,
    textColor3: palette.text.tertiary,
    borderRadius: radii.md,
    borderRadiusSmall: radii.sm,
    fontFamily,
  },
  // n-input-group-label 自带一层 actionColor 底，顶栏 Authorization 上就是那块
  // 白方块。override 走 Input 组件的 self（不是 common），key 名是
  // groupLabelColor —— naive-ui 的 light 主题里 groupLabelColor: actionColor。
  // 和 inputColor 同理：玻璃容器负责那层半透明，标签只留文字。
  Input: {
    groupLabelColor: 'transparent',
  },
}