import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import 'virtual:uno.css'
import { cssVarsToCss, WALLPAPER_OPACITY } from './design-tokens'

// 把 design-tokens 的 CSS 变量注入 :root。
// 这样手写 CSS（如 ChapterPane 的 color-mix）可以直接用 var(--primary-color)，
// 不需要硬编码色值，且改 design-tokens.ts 时这里自动跟随。
const styleEl = document.createElement('style')
styleEl.setAttribute('data-source', 'design-tokens')
styleEl.textContent =
  cssVarsToCss(':root') +
  `
html, body, #app {
  background-color: var(--bg-body);
  min-height: 100%;
}
`
document.head.appendChild(styleEl)

// 全局背景层：public/wallpapers/bg.webp。
// 浅色底需要比深色底更高的 opacity 才看得见，但不能超过 0.45，否则文字对比度崩。
// 默认值在 design-tokens.ts，通过 --wallpaper-opacity 变量下发；
// 读变量而不是内插常量，调试面板改 :root 变量即可实时生效。
const bgStyleEl = document.createElement('style')
bgStyleEl.setAttribute('data-source', 'wallpaper')
bgStyleEl.textContent = `
body::before {
  content: '';
  position: fixed;
  inset: 0;
  background-image: url('/wallpapers/bg.webp');
  background-size: cover;
  background-position: center;
  background-repeat: no-repeat;
  opacity: var(--wallpaper-opacity, ${WALLPAPER_OPACITY});
  pointer-events: none;
  z-index: 0;
}
`
document.head.appendChild(bgStyleEl)

const pinia = createPinia()
const app = createApp(App)

app.use(pinia).mount('#app')