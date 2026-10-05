import type { DownloadTaskState } from './bindings.ts'

// 状态 → 颜色 的单一真相源。
//
// 此前有两份逐字重复的映射：一份产出 CSS 变量引用，一份产出
// UnoCSS 的 arbitrary class 名。两者语义同构，只是出厂包装不同。
// 现在收敛到这里。
//
// 为什么不能只留一份、另一份做封装：
//   class 绑定用的那份，其值是 UnoCSS 在**编译期扫描源码字面量**
//   生成的。若用运行期字符串拼接（把变量插进方括号里），扫描器
//   看不到完整字面量，那条 CSS 就不会生成，样式会直接失效。
//   所以 STATE_TO_TEXT_CLASS 必须保持为手写的静态字面量表。
//
// 注意：不要在注释里写出完整的 arbitrary class 字面量
// （形如 text-[...]），UnoCSS 会把注释也扫进去并生成用不到的规则。
//
// 一致性由类型系统保证：两个 Record 的键都是 DownloadTaskState，
// 任何一个 key 缺失都会编译报错 —— 把「人工同步」变成「编译期保证」。

// 单一真相源：state → CSS 变量引用。
export const STATE_TO_COLOR_VAR: Record<DownloadTaskState, string> = {
  Pending: 'var(--state-neutral)',
  Downloading: 'var(--state-info)',
  Paused: 'var(--state-warning)',
  Cancelled: 'var(--state-cancelled)',
  Completed: 'var(--state-success)',
  Failed: 'var(--state-error)',
}

// 消费点 1：直接返回 var(--state-*)。
// 用于 style 绑定与 color-mix 表达式，必须拿到裸的变量引用。
export function stateColor(state: DownloadTaskState): string {
  return STATE_TO_COLOR_VAR[state]
}

// 消费点 2：静态 Tailwind 字面量，用于 class 绑定。
// 必须手写 —— 详见文件头注释。
export const STATE_TO_TEXT_CLASS: Record<DownloadTaskState, string> = {
  Pending: 'text-[var(--state-neutral)]',
  Downloading: 'text-[var(--state-info)]',
  Paused: 'text-[var(--state-warning)]',
  Cancelled: 'text-[var(--state-cancelled)]',
  Completed: 'text-[var(--state-success)]',
  Failed: 'text-[var(--state-error)]',
}