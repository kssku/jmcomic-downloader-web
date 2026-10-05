<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useMessage } from 'naive-ui'
import { commands, EXPORT_CODE_INCOMPLETE_CHAPTERS } from '../bindings.ts'
import type { ExportedCbz } from '../bindings.ts'

const message = useMessage()

const comicId = ref('')
const exporting = ref(false)
const loadingList = ref(false)
const exports = ref<ExportedCbz[]>([])

/** 上一次导出的错误：区分「缺章」与「系统故障」两种呈现。 */
type ExportError = { kind: 'incomplete' | 'failure'; text: string }
const lastError = ref<ExportError | null>(null)

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

function formatTime(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString()
}

async function loadExports() {
  loadingList.value = true
  try {
    const result = await commands.listExports()
    if (result.status === 'error') {
      message.error(`加载导出列表失败：${result.error.err_message}`)
      return
    }
    exports.value = result.data
  } finally {
    loadingList.value = false
  }
}

async function startExport() {
  const id = comicId.value.trim()
  if (id.length === 0) {
    message.warning('请输入漫画ID')
    return
  }

  exporting.value = true
  lastError.value = null
  try {
    const result = await commands.exportComic(id)
    if (result.status === 'error') {
      const err = result.error
      // 按错误码判类型，不做任何文本匹配——提示语措辞改了也不影响。
      if (err.code === EXPORT_CODE_INCOMPLETE_CHAPTERS) {
        lastError.value = { kind: 'incomplete', text: err.err_message }
        message.warning('未下全，已拒绝导出')
      } else {
        lastError.value = { kind: 'failure', text: err.err_message }
        message.error('导出失败')
      }
      return
    }
    message.success(`已导出 ${result.data.fileName}`)
    comicId.value = ''
    await loadExports()
  } finally {
    exporting.value = false
  }
}

onMounted(loadExports)
</script>

<template>
  <div class="h-full flex flex-col p-4 gap-4">
    <!-- 导出区：与 BatchDownloadPane 同宽（上限 560px），左对齐。 -->
    <div class="export-form flex flex-col">
      <span class="text-sm text-[var(--text-secondary)]">
        把整本已下载的全部章节合成一个 CBZ。缺章会被拒绝。
      </span>
      <n-input
        v-model:value="comicId"
        placeholder="输入漫画ID，例如：55756"
        :disabled="exporting"
        @keyup.enter="startExport"
      />
      <n-button
        type="primary"
        size="large"
        :loading="exporting"
        :disabled="exporting"
        class="mt-4 w-full"
        @click="startExport"
      >
        合成单行本
      </n-button>

      <!-- 错误区分：缺章=警告色（业务约束），系统故障=错误色。 -->
      <div
        v-if="lastError"
        class="mt-3 px-3 py-2 rounded text-sm whitespace-pre-wrap break-all"
        :class="lastError.kind === 'incomplete' ? 'export-warn' : 'export-fail'"
      >
        <div class="font-bold mb-1">
          {{ lastError.kind === 'incomplete' ? '还差若干章未下载' : '导出失败' }}
        </div>
        <div>{{ lastError.text }}</div>
      </div>
    </div>

    <!-- 导出列表区 -->
    <div class="flex flex-col gap-2 flex-1 min-h-0">
      <div class="flex items-center gap-2">
        <span class="text-sm font-bold">已导出的文件</span>
        <n-button size="tiny" :loading="loadingList" :disabled="loadingList" @click="loadExports">
          刷新
        </n-button>
      </div>

      <div v-if="exports.length === 0" class="text-sm text-[var(--text-secondary)] py-2">
        还没有导出文件
      </div>

      <div v-else class="flex flex-col gap-1 overflow-auto">
        <div
          v-for="item in exports"
          :key="item.filePath"
          class="flex items-center gap-3 px-3 py-2 rounded"
          style="background: color-mix(in srgb, var(--bg-card) 80%, transparent)"
        >
          <span class="flex-1 truncate" :title="item.fileName">{{ item.fileName }}</span>
          <span class="text-xs text-[var(--text-secondary)] shrink-0">{{ formatSize(item.fileSize) }}</span>
          <span class="text-xs text-[var(--text-secondary)] shrink-0">{{ formatTime(item.modifiedAt) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 与 BatchDownloadPane 一致的宽度约束。 */
.export-form {
  width: 100%;
  max-width: 560px;
}

.export-warn {
  background: color-mix(in srgb, var(--warning-color, #f0a020) 18%, transparent);
  border: 1px solid color-mix(in srgb, var(--warning-color, #f0a020) 45%, transparent);
  color: var(--text-primary);
}

.export-fail {
  background: color-mix(in srgb, var(--error-color, #d03050) 18%, transparent);
  border: 1px solid color-mix(in srgb, var(--error-color, #d03050) 45%, transparent);
  color: var(--text-primary);
}
</style>
