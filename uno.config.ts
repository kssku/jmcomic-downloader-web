import {
    defineConfig,
    presetAttributify,
    presetIcons,
    presetTypography,
    presetUno,
    presetWebFonts,
    transformerDirectives,
    transformerVariantGroup
} from 'unocss'
import { palette } from './src/design-tokens'

export default defineConfig({
    shortcuts: [
        // ...
    ],
    theme: {
        colors: {
            // 唯一来源：src/design-tokens.ts
            // 不要在这里写字面量色值 —— 否则又会和 theme.ts 分裂。
            primary: {
                DEFAULT: palette.primary.DEFAULT,
                hover: palette.primary.hover,
                pressed: palette.primary.pressed,
                suppl: palette.primary.suppl,
            },
            // 装饰类次色：卡片描边 / 色条 / 徽章边框
            secondary: {
                DEFAULT: palette.secondary.DEFAULT,
                hover: palette.secondary.hover,
                pressed: palette.secondary.pressed,
                suppl: palette.secondary.suppl,
            },
        }
    },
    presets: [
        presetUno(),
        presetAttributify(),
        presetIcons(),
        presetTypography(),
        presetWebFonts({
            fonts: {
                // ...
            },
        }),
    ],
    transformers: [
        transformerDirectives(),
        transformerVariantGroup(),
    ],
})