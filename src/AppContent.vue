<script setup lang="tsx">
import { onMounted, ref, watch } from 'vue'
import { commands, setToken, reconnectEvents } from './bindings.ts'
import { useMessage, useNotification } from 'naive-ui'
import LoginDialog from './dialogs/LoginDialog.vue'
import SearchPane from './panes/SearchPane.vue'
import ChapterPane from './panes/ChapterPane.vue'
import ProgressesPane from './panes/ProgressesPane/ProgressesPane.vue'
import TaskDetailPane from './panes/TaskDetailPane.vue'
import SettingsDialog from './dialogs/SettingsDialog.vue'
import {
  PhInfo,
  PhUser,
  PhClockCounterClockwise,
  PhGearSix,
  PhMagnifyingGlass,
  PhDownloadSimple,
  PhListChecks,
  PhListBullets,
  PhSlidersHorizontal,
  PhFileArchive,
} from '@phosphor-icons/vue'
import AboutDialog from './dialogs/AboutDialog.vue'
import { useStore } from './store.ts'
import LogDialog from './dialogs/LogDialog.vue'
import BatchDownloadPane from './panes/BatchDownloadPane.vue'
import ExportPane from './panes/ExportPane.vue'
import DebugPanel from './components/DebugPanel.vue'
import { CurrentTabName } from './types.ts'

const store = useStore()

const message = useMessage()
const notification = useNotification()

const loginDialogShowing = ref<boolean>(false)
const settingsDialogShowing = ref<boolean>(false)
const aboutDialogShowing = ref<boolean>(false)
const logViewerShowing = ref<boolean>(false)

// 侧栏菜单：paneMenu 里的项切换主区内容，下面的日志/配置/关于打开弹窗。
// 用一个数组驱动渲染，免得模板里写五份几乎相同的按钮。
const paneMenu: { name: CurrentTabName; label: string; icon: unknown }[] = [
  { name: 'progresses', label: '下载列表', icon: PhListBullets },
  { name: 'search', label: '搜索', icon: PhMagnifyingGlass },
  { name: 'chapter', label: '章节详情', icon: PhListChecks },
  { name: 'batch', label: '批量下载', icon: PhDownloadSimple },
  { name: 'export', label: '导出', icon: PhFileArchive },
  { name: 'debug', label: '调参', icon: PhSlidersHorizontal },
]

watch(
  () => store.config,
  async () => {
    if (store.config === undefined) {
      return
    }
    await commands.saveConfig(store.config)
    message.success('保存配置成功')
  },
  { deep: true },
)
watch(
  () => store.config?.token,
  async () => {
    const result = await commands.getUserProfile()
    if (result.status === 'error') {
      console.error(result.error)
      store.userProfile = undefined
      return
    }
    store.userProfile = result.data
    message.success('获取用户信息成功')
  },
)

onMounted(async () => {
  // 屏蔽浏览器右键菜单
  document.oncontextmenu = (event) => {
    event.preventDefault()
  }
  // 获取配置
  store.config = await commands.getConfig()
  // 若本地已保存 token（上次登录过），写入后建立 WebSocket 连接。
  // 即使 token 为空（后端关闭了认证），也照样连接，否则收不到下载进度事件。
  if (store.config.token !== undefined && store.config.token !== '') {
    setToken(store.config.token)
  }
  reconnectEvents()
  // 检查日志目录大小
  const result = await commands.getLogsDirSize()
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  if (result.data > 50 * 1024 * 1024) {
    notification.warning({
      title: '日志目录大小超过50MB，请及时清理日志文件',
      description: () => (
        <>
          <div>
            点击右上角的 <span class="bg-gray-2 px-1">日志</span> 按钮
          </div>
          <div>
            里边有 <span class="bg-gray-2 px-1">打开日志目录</span> 按钮
          </div>
          <div>
            你也可以在里边取消勾选 <span class="bg-gray-2 px-1">输出文件日志</span>
          </div>
          <div>这样将不再产生文件日志</div>
        </>
      ),
    })
  }
})
</script>

<template>
  <div v-if="store.config !== undefined" class="h-screen flex flex-col gap-2 p-2">
    <!-- 顶栏：搜索框 + 登录。跨整个宽度，右侧留给用户信息。 -->
    <div
      class="h-13 flex gap-col-1 px-2 shrink-0 rounded-lg"
      style="
        background: var(--bg-panel);
        backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
        -webkit-backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
        border: var(--border-width) solid
          color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent);
      ">
      <n-input-group class="my-auto">
        <n-input-group-label>Authorization</n-input-group-label>
        <n-input v-model:value="store.config.token" placeholder="手动输入或点击右侧的按钮登录" clearable />
        <n-button type="primary" @click="loginDialogShowing = true">
          <template #icon>
            <n-icon size="20">
              <PhUser />
            </n-icon>
          </template>
          登录
        </n-button>
      </n-input-group>
      <div v-if="store.userProfile !== undefined" class="flex items-center shrink-0">
        <n-avatar round :size="32" :src="store.userProfile.photo" fallback-src="/favicon.png" />
        <span class="whitespace-nowrap">{{ store.userProfile.username }}</span>
      </div>
    </div>

    <div class="flex gap-2 overflow-hidden flex-1">
      <!-- 侧栏：180px 图标+文字菜单。菜单项切主区，弹窗项直接开窗。 -->
      <div
        class="w-[180px] shrink-0 box-border flex flex-col py-4.5 px-3 gap-1 rounded-lg"
        style="
          background: var(--bg-panel);
          backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
          -webkit-backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
          border: var(--border-width) solid
            color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent);
        ">
        <div class="text-lg font-bold px-3 pb-3 select-none">JM Downloader</div>
        <button
          v-for="{ name, label, icon } in paneMenu"
          :key="name"
          class="sidebar-item"
          :class="{ active: store.currentTabName === name }"
          @click="store.currentTabName = name">
          <n-icon :size="18">
            <component :is="icon" />
          </n-icon>
          <span>{{ label }}</span>
        </button>
        <div class="flex-1"></div>
        <button class="sidebar-item" @click="logViewerShowing = true">
          <n-icon :size="18"><PhClockCounterClockwise /></n-icon>
          <span>日志</span>
        </button>
        <button class="sidebar-item" @click="settingsDialogShowing = true">
          <n-icon :size="18"><PhGearSix /></n-icon>
          <span>配置</span>
        </button>
        <button class="sidebar-item" @click="aboutDialogShowing = true">
          <n-icon :size="18"><PhInfo /></n-icon>
          <span>关于</span>
        </button>
      </div>

      <!-- 主区：60% 内容 + 40% 详情。内容列按侧栏菜单切换 pane。 -->
      <div class="flex-1 flex gap-2 overflow-hidden">
        <div
          class="basis-60% shrink-0 box-border overflow-auto flex flex-col rounded-lg"
          style="
            background: var(--bg-panel);
            backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
            -webkit-backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
            border: var(--border-width) solid
              color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent);
          ">
          <progresses-pane v-if="store.currentTabName === 'progresses'" />
          <search-pane v-else-if="store.currentTabName === 'search'" />
          <chapter-pane v-else-if="store.currentTabName === 'chapter'" />
          <BatchDownloadPane v-else-if="store.currentTabName === 'batch'" />
          <ExportPane v-else-if="store.currentTabName === 'export'" />
          <DebugPanel v-else />
        </div>

        <!-- 详情列：选中任务时显示详情，未选中时显示空状态。 -->
        <div
          class="flex-1 box-border overflow-auto flex flex-col rounded-lg"
          style="
            background: color-mix(in srgb, var(--bg-card) calc(var(--detail-alpha) * 100%), transparent);
            backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
            -webkit-backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
            border: var(--border-width) solid
              color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent);
          ">
          <task-detail-pane />
        </div>
      </div>
    </div>

    <login-dialog v-model:showing="loginDialogShowing" />
    <settings-dialog v-model:showing="settingsDialogShowing" />
    <about-dialog v-model:showing="aboutDialogShowing" />
    <log-dialog v-model:showing="logViewerShowing" />
  </div>
</template>

<style scoped>
:global(.n-notification-main__header) {
  @apply break-words;
}

:global(.n-tabs-pane-wrapper) {
  @apply h-full;
}

/* 侧栏菜单项：42px 高、10px 圆角。
   active 态对齐任务行的视觉语言：3px 粉竖条 + 粉字 + 10% 粉底。 */
.sidebar-item {
  @apply flex items-center gap-2.5 px-3 text-sm cursor-pointer select-none border-0 bg-transparent text-left;
  box-sizing: border-box;
  height: 42px;
  border-radius: 10px;
  color: var(--text-secondary);
  position: relative;
  width: 100%;
  font-family: inherit;
  transition: background 0.15s ease, color 0.15s ease;
}

.sidebar-item:hover {
  /* hover 背景 alpha 由调试面板的 --sidebar-hover-alpha 控制。 */
  background: color-mix(in srgb, var(--bg-raised) calc(var(--sidebar-hover-alpha) * 100%), transparent);
}

.sidebar-item.active {
  /* 色相走 --sidebar-active-color（默认=主色），alpha 走 --sidebar-active-alpha。 */
  background: color-mix(in srgb, var(--sidebar-active-color) calc(var(--sidebar-active-alpha) * 100%), transparent);
  color: var(--sidebar-active-color);
  font-weight: 600;
}

.sidebar-item.active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: var(--primary-color);
  border-radius: 3px 0 0 3px;
}
</style>