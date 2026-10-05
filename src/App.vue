<script setup lang="ts">
// This starter template is using Vue 3 <script setup> SFCs
// Check out https://vuejs.org/api/sfc-script-setup.html#script-setup
import AppContent from './AppContent.vue'
import { GlobalThemeOverrides } from 'naive-ui'
import { violetThemeOverrides } from './theme'

// 主题开关：默认走紫色（violetThemeOverrides）。
// 老板原设计（粉色）保留为备选，改这里即可切回。
const USE_VIOLET_THEME = true

// 备选：老板原设计（粉色）。保留不删。
const pinkThemeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#DB547C',
    primaryColorHover: '#E87D9A',
    primaryColorPressed: '#B53C64',
    primaryColorSuppl: '#E87D9A',
    borderRadius: '4px',
    borderRadiusSmall: '3px',
    heightMedium: '32px',
  },
  Button: {
    paddingSmall: '0 8px',
    paddingMedium: '0 12px',
  },
  Radio: {
    buttonColorActive: '#DB547C',
    buttonTextColorActive: '#FFF',
  },
  Dropdown: {
    borderRadius: '5px',
    padding: '6px 2px',
    optionColorHover: '#DB547C',
    optionTextColorHover: '#FFF',
    optionHeightMedium: '28px',
  },
  Tabs: {
    tabTextColorActiveSegment: '#DB547C',
    tabTextColorHoverSegment: '#E87D9A',
    tabColorSegment: '#FFDAE9',
    colorSegment: '#FFFFFFFF',
  },
}

const themeOverrides = USE_VIOLET_THEME ? violetThemeOverrides : pinkThemeOverrides
</script>

<template>
  <n-config-provider :theme-overrides="themeOverrides">
    <n-modal-provider>
      <n-notification-provider placement="bottom-right" :max="3">
        <n-message-provider>
          <app-content />
        </n-message-provider>
      </n-notification-provider>
    </n-modal-provider>
  </n-config-provider>
</template>
