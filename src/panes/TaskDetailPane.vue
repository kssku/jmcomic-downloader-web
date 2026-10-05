<script setup lang="ts">
import { computed } from 'vue'
import { useStore } from '../store.ts'
import { ProgressData } from '../types.ts'

const store = useStore()

// 详情直接从 progresses 里现取，选中态只存 chapterId。
// 任务被删/被取消后 selectedChapterId 可能指向不存在的条目，这里自然回落到空状态。
const detail = computed<ProgressData | undefined>(() => {
  if (store.selectedChapterId === undefined) {
    return undefined
  }
  return store.progresses.get(store.selectedChapterId)
})

const isFailed = computed(() => detail.value?.state === 'Failed')

type LogLine = { level?: string; color?: string; text: string }

// 日志行：真实的 lastError 放最上面。后端暂无日志接口，
// 所以除了这一行以外没有别的数据源 —— 级别色走 --state-* token。
const logLines = computed<LogLine[]>(() => {
  const error = detail.value?.lastError
  const lines: LogLine[] = []
  if (error !== null && error !== undefined && error !== '') {
    lines.push({ level: 'ERROR', color: 'var(--state-error)', text: error })
  }
  return lines
})
</script>

<template>
  <div class="flex flex-col flex-1 overflow-hidden">
    <div
      class="h-10 shrink-0 flex items-center px-4 border-solid border-0 border-b border-[var(--secondary-color)]">
      <span class="text-base font-bold">任务详情</span>
    </div>

    <!-- 空状态：未选中任何任务时显示，不要空白。 -->
    <div v-if="detail === undefined" class="flex-1 flex items-center justify-center">
      <n-empty description="选择左侧任务查看详情" />
    </div>

    <div v-else class="flex-1 overflow-auto p-4 flex flex-col gap-3">
      <div class="flex flex-col gap-1">
        <span class="detail-title">{{ detail.comic.name }}</span>
        <span class="text-xs" style="color: var(--text-secondary)">
          {{ detail.chapterInfo.chapterTitle }}
        </span>
      </div>

      <div class="flex flex-col gap-1.5">
        <div class="progress-track">
          <div
            class="progress-fill"
            :style="{
              width: `${detail.percentage}%`,
              background: isFailed ? 'var(--state-error)' : 'var(--primary-color)',
            }"></div>
        </div>
        <div class="flex justify-between text-xs progress-num">
          <span>{{ detail.downloadedImgCount }}/{{ detail.totalImgCount }}</span>
          <span class="progress-pct">{{ detail.percentage.toFixed(1) }}%</span>
        </div>
      </div>

      <!-- 错误框：仅在失败且有 lastError 时出现。 -->
      <div
        v-if="isFailed && detail.lastError !== null && detail.lastError !== ''"
        class="error-box"
        style="
          background: color-mix(in srgb, var(--state-error) 10%, transparent);
          border: 1px solid color-mix(in srgb, var(--state-error) 35%, transparent);
          color: var(--text-base);
        ">
        {{ detail.lastError }}
      </div>

      <div class="flex flex-col gap-1 flex-1 min-h-0">
        <span class="text-xs font-semibold" style="color: var(--text-secondary)">日志</span>
        <div
          class="flex-1 min-h-20 overflow-auto px-3 py-2 rounded-lg text-xs font-mono log-box"
          style="
            background: color-mix(in srgb, var(--bg-card) 30%, transparent);
            border: var(--border-width) solid
              color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent);
          ">
          <div v-if="logLines.length === 0" style="color: var(--text-tertiary)">暂无日志</div>
          <div v-for="(line, index) in logLines" :key="index" class="log-line whitespace-pre-wrap break-words">
            <span v-if="line.level" class="log-level" :style="{ color: line.color }">{{ line.level }}</span>
            <span class="log-body">{{ line.text }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.detail-title {
  font-size: 17px;
  font-weight: 600;
  line-height: 1.35;
  color: var(--text-base);
  word-break: break-word;
}

/* 进度条：与任务行一致，4px 细轨、无轨道底色。 */
.progress-track {
  width: 100%;
  height: 4px;
  border-radius: 9999px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 9999px;
  transition: width 0.25s ease;
}

.progress-num {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

/* 百分比右对齐，12px。 */
.progress-pct {
  font-size: 12px;
  text-align: right;
}

/* 错误框：10px 圆角，10px 14px 内边距。 */
.error-box {
  box-sizing: border-box;
  padding: 10px 14px;
  border-radius: 10px;
  font-size: 12px;
  line-height: 1.6;
}

/* 日志区：行高 22px，字号 11px。 */
.log-box {
  font-size: 11px;
}

.log-line {
  line-height: 22px;
}

.log-level {
  font-weight: 600;
  margin-right: 6px;
}

/* 正文用 secondary，时间戳/无级别的行走 tertiary。 */
.log-body {
  color: var(--text-secondary);
}
</style>