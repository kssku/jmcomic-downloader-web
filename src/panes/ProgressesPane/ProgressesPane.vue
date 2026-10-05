<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { commands, events, DownloadTaskState, TaskView } from '../../bindings.ts'
import { useStore } from '../../store.ts'
import { ProgressData } from '../../types.ts'
import { normalizeState } from '../../api/state-adapter.ts'
import { PhPause, PhCaretRight, PhTrash, PhWarningCircle } from '@phosphor-icons/vue'

export type ProgressesPaneTabName = 'uncompleted' | 'completed'

const store = useStore()

onMounted(async () => {
  await events.downloadTaskEvent.listen(async ({ payload: { event, data } }) => {
    if (event === 'Create') {
      const { chapterInfo, downloadedImgCount, totalImgCount } = data

      store.progresses.set(chapterInfo.chapterId, {
        ...data,
        percentage: 0,
        indicator: `排队中 ${downloadedImgCount}/${totalImgCount}`,
      })
    } else if (event === 'Update') {
      const { chapterId, state, downloadedImgCount, totalImgCount } = data

      const progressData = store.progresses.get(chapterId)
      if (progressData === undefined) {
        return
      }

      progressData.state = state
      progressData.downloadedImgCount = downloadedImgCount
      progressData.totalImgCount = totalImgCount

      if (state === 'Completed') {
        progressData.chapterInfo.isDownloaded = true
        await syncPickedComic()
        await syncComicInSearch(progressData)
      }

      progressData.percentage = (downloadedImgCount / totalImgCount) * 100
      progressData.indicator = stateIndicator(state, downloadedImgCount, totalImgCount)
    }
  })

  // task-snapshot-event 携带 PascalCase 的活跃任务快照（内存调度器已知的任务）。
  // 它不包含 DB 里的历史任务（如已完成/失败的旧记录），所以不能作为列表唯一数据源。
  // 处理：收到后触发 reloadProgresses()，从 /api/tasks 拉完整列表（含历史）。
  // 快照 payload 本身不用于渲染。
  await events.taskSnapshot.listen(() => {
    void reloadProgresses()
  })

  await reloadProgresses()
})

function stateIndicator(state: string, downloadedImgCount: number, totalImgCount: number): string {
  let indicator = ''
  if (state === 'Pending') indicator = '排队中'
  else if (state === 'Downloading') indicator = '下载中'
  else if (state === 'Paused') indicator = '已暂停'
  else if (state === 'Cancelled') indicator = '已取消'
  else if (state === 'Completed') indicator = '下载完成'
  else if (state === 'Failed') indicator = '下载失败'
  if (totalImgCount !== 0) indicator += ` ${downloadedImgCount}/${totalImgCount}`
  return indicator
}

// 状态列只显示词，不带计数 —— 计数已经在进度数字列里了。
function stateLabel(state: string): string {
  if (state === 'Pending') return '排队中'
  if (state === 'Downloading') return '下载中'
  if (state === 'Paused') return '已暂停'
  if (state === 'Cancelled') return '已取消'
  if (state === 'Completed') return '下载完成'
  if (state === 'Failed') return '下载失败'
  return state
}

// 状态色统一走 token。
// Pending 与 Cancelled 原先同走 --state-neutral，列表里两种状态颜色完全相同。
// 现在 Cancelled 单列 --state-cancelled（冷灰紫），Pending 保留 neutral。
function stateColor(state: DownloadTaskState): string {
  if (state === 'Downloading') return 'var(--state-info)'
  if (state === 'Paused') return 'var(--state-warning)'
  if (state === 'Failed') return 'var(--state-error)'
  if (state === 'Completed') return 'var(--state-success)'
  if (state === 'Cancelled') return 'var(--state-cancelled)'
  return 'var(--state-neutral)'
}

// TaskView（/api/tasks，state 为小写）-> ProgressData（store，state 为 PascalCase）
function taskViewToProgressData(task: TaskView): ProgressData {
  const percentage = task.totalImgCount === 0 ? 0 : (task.doneImgCount / task.totalImgCount) * 100

  return {
    chapterInfo: {
      chapterId: task.chapterId,
      chapterTitle: task.chapterTitle,
      order: task.chapterOrder,
      isDownloaded: task.state === 'completed',
    },
    comic: {
      id: task.comicId,
      name: task.comicTitle,
    },
    state: normalizeState(task.state),
    downloadedImgCount: task.doneImgCount,
    totalImgCount: task.totalImgCount,
    percentage,
    indicator: stateIndicator(normalizeState(task.state), task.doneImgCount, task.totalImgCount),
    retryCount: task.retryCount,
    lastError: task.lastError,
  }
}

async function reloadProgresses() {
  const result = await commands.listTasks({ limit: 500 })
  if (result.status === 'error') {
    console.error(result.error)
    return
  }

  const next = new Map<string, ProgressData>()
  for (const task of result.data.tasks) {
    next.set(task.chapterId, taskViewToProgressData(task))
  }
  store.progresses = next
}

async function syncPickedComic() {
  if (store.pickedComic === undefined) {
    return
  }
  const result = await commands.getSyncedComic(store.pickedComic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  store.pickedComic = result.data
}

async function syncComicInSearch(progressData: ProgressData) {
  if (store.searchResult === undefined) {
    return
  }
  const comic = store.searchResult.docs.find((comic) => comic.id === progressData.comic.id)
  if (comic === undefined) {
    return
  }
  const result = await commands.getSyncedComicInSearch(comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  Object.assign(comic, { ...result.data })
}

const uncompletedCount = computed(
  () => Array.from(store.progresses.values()).filter(({ state }) => state !== 'Completed' && state !== 'Cancelled').length,
)
const completedCount = computed(
  () => Array.from(store.progresses.values()).filter(({ state }) => state === 'Completed').length,
)

// 行式列表：未完成按图片总数降序，已完成按时间新在前。
const rows = computed<[string, ProgressData][]>(() => {
  const entries = Array.from(store.progresses.entries())
  if (store.progressesPaneTabName === 'completed') {
    return entries.filter(([, { state }]) => state === 'Completed')
  }
  return entries
    .filter(([, { state }]) => state !== 'Completed' && state !== 'Cancelled')
    .sort((a, b) => b[1].totalImgCount - a[1].totalImgCount)
})

function selectRow(chapterId: string) {
  store.selectedChapterId = chapterId
}

async function togglePause(state: DownloadTaskState, chapterId: string) {
  if (state === 'Downloading' || state === 'Pending') {
    const result = await commands.pauseDownloadTask(chapterId)
    if (result.status === 'error') console.error(result.error)
  } else if (state === 'Paused') {
    const result = await commands.resumeDownloadTask(chapterId)
    if (result.status === 'error') console.error(result.error)
  }
}

async function cancelTask(chapterId: string) {
  const result = await commands.cancelDownloadTask(chapterId)
  if (result.status === 'error') console.error(result.error)
}
</script>

<template>
  <div class="flex flex-col flex-1 overflow-hidden">
    <!-- 标题行：标题 + 右侧两个切换项（未完成 N / 已完成 N）。 -->
    <div class="h-10 shrink-0 flex items-center px-4 gap-2 border-solid border-0 border-b border-[var(--secondary-color)]">
      <span class="text-base font-bold">下载列表</span>
      <div class="ml-auto flex gap-1">
        <button
          class="tab-chip"
          :class="{ active: store.progressesPaneTabName === 'uncompleted' }"
          @click="store.progressesPaneTabName = 'uncompleted'">
          未完成 {{ uncompletedCount }}
        </button>
        <button
          class="tab-chip"
          :class="{ active: store.progressesPaneTabName === 'completed' }"
          @click="store.progressesPaneTabName = 'completed'">
          已完成 {{ completedCount }}
        </button>
      </div>
    </div>

    <div class="flex-1 overflow-auto px-3 py-2 flex flex-col gap-1.5 task-list">
      <n-empty v-if="rows.length === 0" class="mt-8" description="暂无任务" />
      <div
        v-for="[chapterId, { state, chapterInfo, comic, percentage, downloadedImgCount, totalImgCount }] in rows"
        :key="chapterId"
        class="task-row"
        :class="{ selected: store.selectedChapterId === chapterId }"
        @click="selectRow(chapterId)">
        <div class="flex flex-col min-w-0 flex-1 gap-0.5">
          <span class="row-title" :title="comic.name">{{ comic.name }}</span>
          <span class="row-subtitle">{{ chapterInfo.chapterTitle }}</span>
        </div>

        <!-- 进度条：4px 细轨，无轨道底，只留填充 -->
        <div class="shrink-0 progress-track">
          <div
            class="progress-fill"
            :style="{
              width: `${percentage}%`,
              background: state === 'Failed' ? 'var(--state-error)' : 'var(--primary-color)',
            }"></div>
        </div>

        <span class="shrink-0 text-xs progress-num">
          {{ downloadedImgCount }}/{{ totalImgCount }}
        </span>

        <!-- 状态 pill -->
        <span
          class="shrink-0 state-pill"
          :style="{
            color: stateColor(state),
            background: `color-mix(in srgb, ${stateColor(state)} calc(var(--pill-alpha) * 100%), transparent)`,
          }">
          {{ stateLabel(state) }}
        </span>

        <div class="shrink-0 flex gap-0.5 items-center" @click.stop>
          <button
            v-if="state === 'Downloading' || state === 'Pending' || state === 'Paused'"
            class="ghost-btn"
            :title="state === 'Paused' ? '继续' : '暂停'"
            @click="togglePause(state, chapterId)">
            <n-icon :size="15">
              <PhPause v-if="state === 'Downloading' || state === 'Pending'" />
              <PhCaretRight v-else />
            </n-icon>
          </button>
          <button class="ghost-btn" :title="state === 'Failed' ? '重试' : '取消'" @click="cancelTask(chapterId)">
            <n-icon :size="15">
              <PhTrash v-if="state !== 'Failed'" />
              <PhWarningCircle v-else />
            </n-icon>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tab-chip {
  @apply px-2.5 py-0.75 rounded-full text-xs cursor-pointer border-0 select-none;
  background: transparent;
  color: var(--text-secondary);
  font-family: inherit;
}

.tab-chip:hover {
  background: color-mix(in srgb, var(--primary-color) 12%, transparent);
  color: var(--text-base);
}

.tab-chip.active {
  background: var(--bg-selected);
  color: var(--text-base);
  font-weight: 600;
}

/* 行列表：行变高后行间距收一档，一行一个单元。 */
.task-list {
  gap: 6px;
}

/* 行：70px 高，横向 20px / 纵向 12px 内边距。
   未选中用 --bg-raised，选中用 --bg-selected + 左侧 3px 粉竖条。 */
.task-row {
  @apply flex items-center gap-3 cursor-pointer select-none shrink-0;
  /* box-border：高度含边框与内边距，否则会实测溢出（踩过两次） */
  box-sizing: border-box;
  height: 70px;
  padding: 12px 20px;
  border-radius: 14px;
  position: relative;
  /* 背景 alpha 由调试面板的 --row-alpha 控制（默认 0.44，等于 bg-raised 档位）。 */
  background: color-mix(in srgb, var(--bg-raised) calc(var(--row-alpha) * 100%), transparent);
  backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
  -webkit-backdrop-filter: blur(var(--glass-blur, 2px)) saturate(var(--glass-saturate, 1.2));
  border: var(--border-width) solid
    color-mix(in srgb, var(--border-color) calc(var(--border-alpha) * 100%), transparent);
}

.task-row.selected {
  background: var(--bg-selected);
  padding-left: 23px;
}

.task-row.selected::before {
  content: '';
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: var(--primary-color);
  border-radius: 14px 0 0 14px;
}

/* 漫画名 15px / 字重 500；章节号保持 11px，tertiary 色。 */
.row-title {
  font-size: 15px;
  font-weight: 500;
  line-height: 1.3;
  color: var(--text-base);
  text-overflow: ellipsis;
  white-space: nowrap;
  overflow: hidden;
}

.row-subtitle {
  font-size: 11px;
  line-height: 1.3;
  color: var(--text-tertiary);
  text-overflow: ellipsis;
  white-space: nowrap;
  overflow: hidden;
}

/* 进度条：4px 细轨，去掉轨道底色，只留填充。 */
.progress-track {
  width: 150px;
  height: 4px;
  border-radius: 9999px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 9999px;
  transition: width 0.25s ease;
}

/* 进度数字：tabular-nums 让数字等宽，扫读时不跳。 */
.progress-num {
  width: 62px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
  text-align: right;
}

/* 状态 pill：14% 状态色底 + 状态色文字。 */
.state-pill {
  min-width: 56px;
  padding: 2px 10px;
  border-radius: 9999px;
  font-size: 11px;
  font-weight: 500;
  text-align: center;
  white-space: nowrap;
  box-sizing: border-box;
}

/* 按钮：ghost 小圆钮，默认无底无边，hover 才出现。 */
.ghost-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 0;
  border-radius: 9999px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  font-family: inherit;
  transition: background 0.15s ease, color 0.15s ease;
}

.ghost-btn:hover {
  background: color-mix(in srgb, var(--primary-color) 12%, transparent);
  color: var(--primary-color);
}
</style>