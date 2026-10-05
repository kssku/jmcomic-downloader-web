import type { DownloadTaskState } from '../bindings.ts'

// `/api/tasks` 与 WebSocket 两条路径的 state 字面量不一致，这里做归一。
//
// 背景（已核实）：
//   - WebSocket 事件（download-task-event / task-snapshot-event）的 state 来自
//     `download_manager::DownloadTaskState` 枚举直序列化，枚举无 #[serde(rename_all)]，
//     所以是 PascalCase："Pending" / "Downloading" / ...
//   - `GET /api/tasks` 的 state 来自 `DbTaskState::as_str()`，是小写：
//     "pending" / "downloading" / ...
//
// bindings.ts 的 `DownloadTaskState` 标的是 WebSocket 侧（PascalCase），
// 且全前端的状态比较（UncompletedProgresses / CompletedProgresses /
// ProgressesPane）都用 PascalCase 字面量。所以从 /api/tasks 拿到的记录
// **写入 Pinia 之前**必须过这一层，否则：
//   - state !== 'Completed' 对小写 "completed" 恒为 true
//     -> 已完成任务永远留在「未完成」tab
//   - state === 'Completed' 恒为 false -> 「已完成」tab 永远空
// 这是静默失效（不报错、类型检查也过），所以适配层是必需的。

const STATE_TO_PASCAL: Record<string, DownloadTaskState> = {
  pending: 'Pending',
  downloading: 'Downloading',
  paused: 'Paused',
  cancelled: 'Cancelled',
  completed: 'Completed',
  failed: 'Failed',
}

/**
 * 把 `/api/tasks` 返回的小写 state 归一成 PascalCase。
 *
 * 未知状态直接抛错，不静默归类 —— 后端若新增状态（如 `retrying`），
 * 宁可显式炸掉，也不要让任务悄悄落进错误的 tab。
 * 后端当前状态枚举只有 6 个（`DbTaskState` / `DownloadTaskState` 一致）。
 */
export function normalizeState(s: string): DownloadTaskState {
  const v = STATE_TO_PASCAL[s.toLowerCase()]
  if (!v) throw new Error(`Unknown task state from /api/tasks: ${s}`)
  return v
}
