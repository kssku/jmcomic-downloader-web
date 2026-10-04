> 这些测试针对的是 **`src-server/static/index.html`** —— 后端用 `include_str!` 嵌进二进制的那个单文件控制台，
> **不是**仓库根 `src/` 里的 Vue 应用。两者是独立前端，别搞混。

## 跑

```bash
pnpm test:console            # 全部 30 项
pnpm test:console:mutation   # 变异测试：证明 test-ws 真能抓到回归
```

或直接：

```bash
src-server/tests/frontend/run-all.sh
```

## 文件

| 文件 | 作用 |
|---|---|
| `harness.cjs` | 测试桩。**每次运行都从 `index.html` 现抽 `<script>` 块执行**，不依赖任何复制副本 |
| `test-behavior.cjs` | 18 项：分页、筛选、失败区分空态、竞态、就地更新 |
| `test-ws.cjs` | 12 项：走**真实 `ws.onmessage` 分支**的 WS 快照行为 |
| `mutation-check.sh` | 把 `onmessage` 里的 `loadTasks()` 换回 `state.tasks = payload`，期望测试变红 |
| `run-all.sh` | 依次跑上面两个测试文件 |

## 为什么有变异测试

一个全绿的测试**不能证明它有效**——它必须在缺陷复现时变红。

`mutation-check.sh` 把修复回退成出问题的写法，跑 `test-ws.cjs`，期望：

- **exit 1**（干净失败，不是崩溃）→ 测试有效
- **exit 0**（仍全绿）→ 测试无效，抓不到这个缺陷

脚本用 `trap` 保证无论成败都还原源码，并在结束时校验工作区干净。

## 背景

这 30 项覆盖的是 `static/index.html` 里三个修复：

1. **WS 快照不再覆盖分页** —— 快照是「所有未完结任务」全量列表（排序=内存优先+`created_at ASC`），
   而列表走分页 API（`updated_at DESC` + 筛选）。直接 `state.tasks = payload` 会把第 2 页撑成全量、
   冲掉筛选。改为只用快照当「有变化」的信号，重新拉当前页。
2. **`loadTasks` 加 `loadSeq` 防竞态** —— 慢响应后到会覆盖新数据。
3. **加载失败区分于空态** —— 原先失败只 `log()`，界面停在「没有匹配的任务」，
   用户分不清「真的没有」和「请求挂了」。

## 注意

- **`index.html` 是编译期 `include_str!` 嵌进二进制的**。改完要 `docker build` 才生效，重启容器没用。
- 这些是 **CommonJS（`.cjs`）**：仓库根 `package.json` 有 `"type": "module"`，`.js` 会被当 ESM。
- 测试**不依赖网络、不依赖数据库**，纯桩。