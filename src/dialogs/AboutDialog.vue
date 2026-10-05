<script setup lang="ts">
import { commands } from '../bindings.ts'
import { useStore } from '../store.ts'
import { ref, onMounted } from 'vue'
import icon from '/favicon.png'

const store = useStore()

const showing = defineModel<boolean>('showing', { required: true })
const version = ref('')

onMounted(async () => {
  if (store.serverInfo === undefined) {
    // 同 SettingsDialog：callDirect 抛异常，不兜住的话下面 version.value
    // 那行根本执行不到，版本号永远空白。
    try {
      store.serverInfo = (await commands.getServerInfo()) ?? undefined
    } catch (e) {
      console.error('获取服务器信息失败', e)
      store.serverInfo = undefined
    }
  }
  version.value = store.serverInfo?.version ?? ''
})
</script>

<template>
  <n-modal v-model:show="showing">
    <n-dialog :showIcon="false" @close="showing = false">
      <div class="flex flex-col items-center gap-row-6">
        <img :src="icon" alt="icon" class="w-32 h-32" />
        <div class="text-center text-[var(--text-secondary)] text-xs">
          <div>
            如果本项目对你有帮助，欢迎来
            <n-a href="https://github.com/kssku/jmcomic-downloader-web" target="_blank">GitHub</n-a>
            点个Star⭐支持！
          </div>
          <div class="mt-1">你的支持是我持续更新维护的动力🙏</div>
        </div>
        <div class="flex flex-col w-full gap-row-3 px-6">
          <div class="flex items-center justify-between py-2 px-4 bg-[var(--bg-raised)] rounded-lg">
            <span class="text-[var(--text-secondary)]">软件版本</span>
            <div class="font-medium">v{{ version }}</div>
          </div>
          <div class="flex items-center justify-between py-2 px-4 bg-[var(--bg-raised)] rounded-lg">
            <span class="text-[var(--text-secondary)]">开源地址</span>
            <n-a href="https://github.com/kssku/jmcomic-downloader-web" target="_blank">GitHub</n-a>
          </div>
          <div class="flex items-center justify-between py-2 px-4 bg-[var(--bg-raised)] rounded-lg">
            <span class="text-[var(--text-secondary)]">问题反馈</span>
            <n-a href="https://github.com/kssku/jmcomic-downloader-web/issues" target="_blank">GitHub Issues</n-a>
          </div>
        </div>
        <div class="flex flex-col text-xs items-center text-[var(--text-tertiary)]">
          <div>
            基于
            <n-a href="https://github.com/lanyeeee/jmcomic-downloader" target="_blank">lanyeeee/jmcomic-downloader</n-a>
            与
            <n-a href="https://github.com/lanyeeee/picacomic-downloader" target="_blank">picacomic-downloader</n-a>
            改造
          </div>
          <div>
            Copyright © 2024-2025
            <n-a href="https://github.com/lanyeeee" target="_blank">lanyeeee</n-a>
          </div>
          <div>
            Released under
            <n-a href="https://github.com/kssku/jmcomic-downloader-web/blob/main/LICENSE" target="_blank">
              MIT License
            </n-a>
          </div>
        </div>
      </div>
    </n-dialog>
  </n-modal>
</template>
