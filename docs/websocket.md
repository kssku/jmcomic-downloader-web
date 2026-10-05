# WebSocket 协议参考

> 来源：`src-server/src/api/ws.rs`（约 120 行）、`src-server/src/event_bus.rs`、`src-server/src/events.rs`、前端 `src/bindings.ts`。
> 本文件描述**当前代码事实**。

端点：`GET /api/ws`（受保护路由器，走 `require_auth`）。

---

## 一、为什么是 WebSocket

原桌面版用 `tauri::event` 把后端事件推给 WebView。Web 版改成 WebSocket：前端连上 `/api/ws` 后，后端把 `EventBus` 上的事件**原样转发**过去。

设计目标是**保住前端的 `listen(topic)` 用法** —— 前端按 `topic` 分发，语义与原来的 `events.yyy.listen(cb)` 一致。

---

## 二、认证

WebSocket **不能设自定义请求头**（浏览器 `WebSocket` 构造器只接受 URL 和子协议），所以凭据走**查询串**：

```
ws://<host>:<port>/api/ws?token=<JM_AUTH_TOKEN>
```

后端由 `require_auth` 的 `?token=` fallback 分支处理。

**`JM_AUTH_DISABLED` 默认 `true`**，此时不带 token 也能连上。测试环境即处于关闭状态。

---

## 三、报文格式

### 3.1 服务端 → 客户端

每条消息是一个 JSON 文本帧，形状固定为：

```json
{ "topic": "<主题名>", "payload": { ... } }
```

对应 `event_bus.rs` 的 `BusMessage { topic: String, payload: serde_json::Value }`。

**事件在入队时就序列化成 `(topic, json)`** —— 避免每个连接各序列化一遍。订阅者数为 0 时直接跳过序列化（`emit` 的早退分支）。

### 3.2 客户端 → 服务端

**只接受两种**：

| 帧 | 处理 |
|---|---|
| 文本 `ping`（内容任意，当前无语义） | **忽略**（源码注释：文本消息当前没有语义，忽略即可） |
| WebSocket `Ping` 控制帧 | 回 `Pong`（原样带回 payload） |

⚠️ **注意这里有个容易误记的点**：`ws.rs` 的模块注释里写「只接受文本 `ping` 做保活探测」，但**实际的代码路径**是：文本帧直接落到 `Some(Ok(_)) => {}` 被忽略；**真正被响应的是 WebSocket 协议层的 `Ping` 控制帧**，回一个 `Pong`。两个方向都有心跳，见下节。

---

## 四、连接生命周期

### 4.1 第一步：补发任务快照（**顺序很重要**）

连上后**立刻**发一条 `task-snapshot-event`：

```json
{
  "topic": "task-snapshot-event",
  "payload": [ /* Vec<DownloadTaskEvent> */ ]
}
```

来源：`app.get_download_manager().task_snapshot()`。

源码注释解释了为什么必须**先**发快照：

> 前端收到快照后会把任务列表**整体替换**，随后到达的 Update 事件才有正确的基线。

**顺序颠倒的后果**：如果先转发实时事件再发快照，快照会用一份**陈旧的全量列表覆盖**掉刚收到的增量更新 —— 前端表现为「刚连上时进度回跳」。

### 4.2 第二步：实时事件转发

发完快照后进入 `tokio::select!` 主循环，订阅 `app.events().subscribe()`，把每条 `BusMessage` 序列化后转发。

### 4.3 心跳（两个方向）

| 方向 | 机制 | 间隔 |
|---|---|---|
| 服务端 → 客户端 | WebSocket `Ping` 控制帧（空 payload） | **30 秒**（`HEARTBEAT_INTERVAL`） |
| 客户端 → 服务端 | WebSocket `Ping` 控制帧，服务端回 `Pong` | 由客户端决定 |

**为什么需要**（源码注释原文）：让中间的反向代理（飞牛的 Nginx 等）不会掐断长连接。

**实现细节**：心跳定时器和事件转发**放在同一个 `select!` 里**，避免额外开一个 tokio task。`tokio::time::interval` 的**第一次 tick 会立即触发**，所以代码在进循环前先 `heartbeat.tick().await` **吃掉这一次**，免得刚连上就发一个 Ping。

### 4.4 断开与背压

| 情况 | 行为 |
|---|---|
| 客户端发 `Close` 或流结束 | `break`，关闭连接 |
| `sender.send(...)` 返回 Err | `break`，关闭连接（对端已走） |
| 广播通道 `Closed` | `break` |
| **广播通道 `Lagged(skipped)`** | **只 `tracing::warn!` 并跳过这一条，不掐断连接** |

**`Lagged` 不掐断是刻意的**：慢客户端只丢自己的消息，不阻塞发布方（`event_bus.rs` 的设计要点）。广播通道容量 `CHANNEL_CAPACITY = 1024` —— 慢客户端积压超过这个数就开始丢旧消息。

⚠️ **客户端要能容忍丢事件**。丢了之后没有重传机制；唯一的补救是**重新连接**（会重新收到一次全量快照）。这也是 `task-snapshot-event` 存在的第二个理由。

### 4.5 前端的重连策略

前端 `src/bindings.ts` 的 WS 客户端负责：

1. 首次连接发 `"ping"` 文本帧；
2. **25 秒**心跳（比服务端的 30 秒略短，保证先于服务端 Ping 发出）；
3. `onclose` 触发**带退避的重连**；
4. 重连成功后，对已注册的 `subscribe()` 回调**重放 `lastPayload`** —— 让订阅者在重连窗口期不会看到空数据。

> ⚠️ 客户端 25s / 服务端 30s 这两个数字**必须保持 25 < 30**。把客户端调到大于 30 会让服务端的 Ping 先到，某些代理下会被判定为单向心跳而提前断连。

---

## 五、5 个 Topic 全表

集中定义在 `src-server/src/event_bus.rs` 的 `topics` 模块。

| # | 常量 | 字符串值 | Payload 类型 | 前端是否订阅 |
|---|------|----------|--------------|--------------|
| 1 | `DOWNLOAD_TASK` | `download-task-event` | `DownloadTaskEvent`（enum） | ✅ 订阅 |
| 2 | `TASK_SNAPSHOT` | `task-snapshot-event` | `Vec<DownloadTaskEvent>` | ✅ 订阅 |
| 3 | `LOG` | `log-event` | `LogEvent` | ✅ 订阅 |
| 4 | `DOWNLOAD_ALL_FAVORITES` | `download-all-favorites-event` | （见 §5.4） | ❌ **未连接** |
| 5 | `UPDATE_DOWNLOADED_COMICS` | `update-downloaded-comics-event` | （见 §5.5） | ❌ **未连接** |

⚠️ **`event_bus.rs` 的模块注释写「3 个 topic」，但常量实际有 5 个** —— 注释是 `TASK_SNAPSHOT` 加入之前写的，没有同步更新。

⚠️ **前端 `EventMap`（`src/bindings.ts` 约 483 行）只列出前 3 个**。后 2 个 topic **后端会发（或有能力发），但前端没有任何订阅点** —— 登记为 legacy。

### 5.1 `download-task-event`

Payload 是 `DownloadTaskEvent` 枚举，`#[serde(tag = "event", content = "data")]` —— 即**内部标签**形式，`event` 字段区分变体，`data` 字段装内容：

```json
{ "topic": "download-task-event",
  "payload": { "event": "Create", "data": { ... } } }

{ "topic": "download-task-event",
  "payload": { "event": "Update", "data": { ... } } }
```

#### `Create` 变体（`#[serde(rename_all = "camelCase")]`）

| 字段 | 类型 |
|---|---|
| `state` | `DownloadTaskState`（**PascalCase 字符串**） |
| `comic` | `Comic`（boxed） |
| `chapterInfo` | `ChapterInfo`（boxed） |
| `downloadedImgCount` | `u32` |
| `totalImgCount` | `u32` |

#### `Update` 变体

| 字段 | 类型 | 备注 |
|---|---|---|
| `chapterId` | `String` | |
| `state` | `DownloadTaskState` | **PascalCase 字符串** |
| `downloadedImgCount` | `u32` | |
| `totalImgCount` | `u32` | |
| `retryCount` | `Option<u32>` | `#[serde(default, skip_serializing_if = "Option::is_none")]` |
| `lastError` | `Option<String>` | 同上；成功时为 `None`（**字段整个不出现**） |

源码注释说明：`retryCount` 是**后加的字段**，旧前端不认识会直接忽略，所以加字段是**向后兼容**的。

⚠️ **`Update` 是高频事件**（每张图片一次），20 并发下非常密集。`event_bus.rs` 为此提供 `emit_throttled(topic, key, min_interval, event)` —— 同一个 `key` 在 `min_interval` 内只发第一条，按 key 合并。另有 `prune_throttle(older_than)` 清理节流表，防止长期运行无限增长。

### 5.2 `task-snapshot-event`

Payload：`Vec<DownloadTaskEvent>`（直接是数组，**没有外层包装**）。

**发送时机**：仅在**连接建立时一次**（`ws.rs` 的 `handle_socket` 开头）。之后不再补发。

### 5.3 `log-event`

Payload 是 `LogEvent` 结构体（`#[serde(rename_all = "camelCase")]`）：

| 字段 | 类型 | 备注 |
|---|---|---|
| `timestamp` | `String` | |
| `level` | `LogLevel` | |
| `fields` | `HashMap<String, serde_json::Value>` | 结构化日志字段 |
| `target` | `String` | |
| `filename` | `String` | |
| `line_number` | `i64` | ⚠️ **显式 `#[serde(rename = "line_number")]`，是 snake_case** |

⚠️ `line_number` 是**唯一**一个在 `LogEvent` 里保持 snake_case 的字段。同结构体里其他 5 个都是 camelCase。**不要顺手统一** —— 前端读取点写的是 `line_number`，改了会静默变 `undefined`。

WebSocket 推的是**增量**日志。初始加载（历史 N 行）走 REST：`POST /api/logs` 带 `{ "tail": 500 }`，或 `GET /api/logs?tail=200`。见 [`API.md`](./API.md)。

### 5.4 `download-all-favorites-event`（未连接）

常量存在，`topics::DOWNLOAD_ALL_FAVORITES`。**前端 `EventMap` 里没有它**，没有任何订阅点。

**登记为 legacy**：在决定「补前端订阅」还是「删后端 topic」之前保持现状。注意**删 topic 字符串会让已连接客户端的对应事件静默丢失** —— 分发是按字符串查找，找不到就丢，不报错、不重试。

### 5.5 `update-downloaded-comics-event`（未连接）

同上。常量 `topics::UPDATE_DOWNLOADED_COMICS`，前端无订阅点。

**注意它与 REST 的 `/api/sync/comic` 功能重叠** —— 后者是「下载完成后刷新详情/搜索列表里的已下载标记」的**当前实现路径**（`src-server/src/api/routes.rs` 的 `sync_comic` / `sync_comic_in_search`，自 `068f36c^` 恢复）。这两个 topic 很可能是**旧推送路径的残留**，被 REST 拉取取代了。

---

## 六、破坏性提醒（改协议前必读）

1. **topic 字符串是运行期按值查找的**。改任何一个字符串 = 已连接客户端的事件**静默丢弃** —— 不报错、不重连、不重试，前端表现为「某个面板永远不更新」。
2. **`state` 在 WS 里是 PascalCase，在 REST 里是 lowercase**。归一点在前端 `taskViewToProgressData()`（`src/panes/ProgressesPane/ProgressesPane.vue`）。改任一侧都必须同时改前端。
3. **`event`/`data` 内部标签名来自 `#[serde(tag = "event", content = "data")]`**（`src-server/src/events.rs`）。改 `tag`/`content` 的值 = 前端所有 `payload.event === 'Update'` 判断同时失效。
4. **`Lagged` 会丢事件且不通知客户端**。任何依赖「事件一定送达」的新功能都必须自带快照/对账机制。
5. **快照必须最先发**（§4.1）。把快照挪到事件循环之后会引入「连上瞬间进度回跳」。

---

## 七、相关文件

| 文件 | 作用 |
|---|---|
| `src-server/src/api/ws.rs` | WebSocket handler：快照、转发、心跳 |
| `src-server/src/event_bus.rs` | `EventBus` + `topics` 常量 + 节流 |
| `src-server/src/events.rs` | `DownloadTaskEvent` / `LogEvent` 结构体 |
| `src/bindings.ts` | 前端 WS 客户端 + `EventMap`（约 483 行） |
| `src-server/tests/frontend/test-ws.cjs` | WS 行为测试（12 项） |