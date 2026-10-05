import { DownloadTaskEvent } from './bindings'

export type CurrentTabName = 'progresses' | 'search' | 'chapter' | 'batch' | 'debug'

/**
 * 进度列表里展示一条漫画所需的最小字段。
 *
 * 刻意**不**用完整的 `Comic`：进度列表的数据来源有两个——
 * WebSocket 的 `Create` 事件（带完整 `Comic`）和 `GET /api/tasks`
 * （只有 `comicId` + `comicTitle`）。列表页只展示 `comic.name`，
 * 「重下」路径只传 `comic.id` 给 `POST /api/download/by-id`，
 * 由后端按 id 重新取详情。绑完整 `Comic` 会逼 `TaskView` 伪造字段。
 */
export type ProgressComicRef = {
  id: string
  name: string
}

export type ProgressData = Omit<Extract<DownloadTaskEvent, { event: 'Create' }>['data'], 'comic'> & {
  comic: ProgressComicRef
  percentage: number
  indicator: string
  /**
   * 仅 `GET /api/tasks` 路径带这两个字段（WS 事件 payload 里没有）。
   * `retryCount` 默认 0，`lastError` 默认 null —— 详情面板据此显示错误框。
   */
  retryCount?: number
  lastError?: string | null
}
