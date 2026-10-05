# HTTP API 参考

> 来源：`src-server/src/api/routes.rs`（599 行）为唯一权威。
> 本文件是**盘点产出**，不是设计文档 —— 只描述当前代码事实，不描述「应该怎样」。
> 前端调用面：`src/bindings.ts`。

前缀：所有路由挂载在 `/api` 下（见 `src-server/src/main.rs` 的 `nest("/api", api)`）。
WebSocket 是 `/api/ws`，协议另见 [`websocket.md`](./websocket.md)。

---

## 一、认证

| 层 | 说明 |
|---|---|
| 开关 | `JM_AUTH_DISABLED`，**默认 `true`**（局域网无认证）。测试环境即处于关闭状态 |
| 中间件 | `require_auth`，只挂在 **protected** 路由器上（`routes.rs:107-110`） |
| 凭据形式 1 | `Authorization: Bearer <JM_AUTH_TOKEN>` |
| 凭据形式 2 | `?token=<JM_AUTH_TOKEN>` 查询串（`require_auth` 内的 fallback） |

**公开路由不受中间件约束**，只有两条：`/health`、`/auth/check`。

---

## 二、完整路由表（28 条）

`Auth` 列含义：**公开** = 不挂中间件；**受保护** = 挂 `require_auth`（但 `JM_AUTH_DISABLED=true` 时等于放行）。

| # | 方法 | 路径 | Handler | Auth | 响应类型 |
|---|------|------|---------|------|----------|
| 1 | GET | `/api/health` | `health` | 公开 | `json!({"status":"ok"})` ⚠️无类型 |
| 2 | GET | `/api/auth/check` | `auth_check` | 公开 | `json!({"ok":true})` ⚠️无类型 |
| 3 | GET | `/api/config` | `get_config` | 受保护 | `Json<Config>` |
| 4 | POST | `/api/config` | `save_config` | 受保护 | `Json<()>` |
| 5 | GET | `/api/search` | `search_comic` | 受保护 | `Json<SearchResult>` |
| 6 | POST | `/api/search` | `post_search` | 受保护 | `Json<SearchResult>` |
| 7 | GET | `/api/comic/:comic_id` | `get_comic` | 受保护 | `Json<Comic>` |
| 8 | POST | `/api/comic` | `post_comic` | 受保护 | `Json<Comic>` |
| 9 | POST | `/api/download/task` | `create_download_task` | 受保护 | `Json<()>` |
| 10 | POST | `/api/download/task/:chapter_id/pause` | `pause_download_task` | 受保护 | `Json<()>` |
| 11 | POST | `/api/download/task/:chapter_id/resume` | `resume_download_task` | 受保护 | `Json<()>` |
| 12 | POST | `/api/download/task/:chapter_id/cancel` | `cancel_download_task` | 受保护 | `Json<()>` |
| 13 | POST | `/api/download/comic` | `download_comic` | 受保护 | `Json<()>` |
| 14 | POST | `/api/download/by-id` | `download_by_id` | 受保护 | `Json<DownloadByIdResult>` |
| 15 | GET | `/api/download/tasks` | `list_download_tasks` | 受保护 | `Json<Vec<DownloadTaskEvent>>` |
| 16 | GET | `/api/tasks` | `query_tasks` | 受保护 | `Json<TaskListView>` |
| 17 | GET | `/api/tasks/stats` | `task_stats` | 受保护 | `Json<TaskStats>` |
| 18 | POST | `/api/tasks/purge` | `purge_tasks` | 受保护 | `Json<PurgeResult>` |
| 19 | GET | `/api/tasks/:chapter_id` | `get_task` | 受保护 | `Json<TaskDetailView>` |
| 20 | DELETE | `/api/tasks/:chapter_id` | `delete_task` | 受保护 | `json!({"ok":true})` ⚠️无类型 |
| 21 | POST | `/api/tasks/:chapter_id/retry` | `retry_task` | 受保护 | `Json<RetryResult>` |
| 22 | POST | `/api/export/comic/:comic_id` | `export_comic_cbz` | 受保护 | `Json<ExportResult>` |
| 23 | GET | `/api/export/list` | `list_exported_cbz` | 受保护 | `Json<Vec<ExportedCbz>>` |
| 24 | GET | `/api/catalog/status` | `catalog_status` | 受保护 | `json!({...})` ⚠️无类型 |
| 25 | POST | `/api/catalog/trigger` | `catalog_trigger` | 受保护 | `json!({...})` ⚠️无类型 |
| 26 | POST | `/api/sync/comic` | `sync_comic` | 受保护 | `Json<Comic>` |
| 27 | POST | `/api/sync/comic-in-search` | `sync_comic_in_search` | 受保护 | `Json<ComicInSearch>` |
| 28 | GET+POST | `/api/logs/size` | `get_logs_dir_size` / `post_logs_dir_size` | 受保护 | `Json<u64>` |
| 29 | GET+POST | `/api/logs` | `get_logs` / `post_logs` | 受保护 | `Json<Vec<String>>` |
| 30 | POST | `/api/server/info` | `post_server_info` | 受保护 | `json!({...})` ⚠️无类型 |
| 31 | GET | `/api/ws` | `ws::handler` | 受保护 | WebSocket 升级 |

> **计数口径**：`routes.rs` 里 `.route(...)` 调用共 **28 次**；其中 `/logs/size`、`/logs` 各挂 GET+POST 两个方法，`/tasks/:chapter_id` 挂 GET+DELETE 两个方法。按 **(方法,路径) 对** 展开是 31 条。上文表格按 route 调用逐条列出，共 31 行 —— 两种口径都对，取决于你要数「路由」还是「端点」。

### 路由顺序陷阱（已注释在源码里）

`/tasks/purge` 是**静态段**，在 `matchit` 路由树里静态优先于 `/tasks/:chapter_id`，所以 `purge` 不会被当成 `chapter_id`。两者可以共存，不需要额外规避。

**同一条路径上挂多个方法必须一次 `route()` 注册完** —— axum 对同一路径重复 `route()` 会在**启动时 panic**。`/tasks/:chapter_id` 的 `get(...).delete(...)` 因此写成一行。

---

## 三、请求体格式

### 3.1 无请求体（路径参数或纯触发）

`/download/task/:chapter_id/{pause,resume,cancel}`、`/tasks/:chapter_id`（GET/DELETE）、`/tasks/:chapter_id/retry`、`/export/comic/:comic_id`、`/export/list`、`/catalog/status`、`/catalog/trigger`、`/server/info`（POST 但**不接受 body**）、`/logs/size`（POST 但**忽略 body**）。

### 3.2 有请求体

全部在边界上 `#[serde(rename_all = "camelCase")]`，即**前端发 camelCase**。

| 端点 | 结构体 | 字段 |
|---|---|---|
| `POST /config` | `Config` | 见 `src-server/src/types.rs` |
| `POST /search` | `SearchQuery` | `keyword: String`、`sort: Option`、`page: Option`、`categories: Vec`（`#[serde(default)]`） |
| `POST /comic` | `Comic` | 20 个必填字段（见下方「踩坑」） |
| `POST /download/task` | `CreateTaskRequest` | `comic: Comic`、`chapterId: String` |
| `POST /download/comic` | `ComicIdRequest` | `comicId: String` |
| `POST /download/by-id` | `DownloadByIdRequest` | `comicId: String`、`chapterId: Option<String>`（`#[serde(default)]`） |
| `POST /tasks/purge` | `PurgeRequest`（**整体可选**） | `retentionDays: Option<i64>`（`#[serde(default)]`）；不传 body 等价于不传该字段 |
| `POST /sync/comic` | `ComicWrapper<Comic>` | `{ "comic": { ...Comic } }` |
| `POST /sync/comic-in-search` | `ComicWrapper<ComicInSearch>` | `{ "comic": { ...ComicInSearch } }` |
| `POST /logs` | `LogsQuery` | `tail: usize`，`#[serde(default = "default_tail")]` → **默认 500** |

> `GET /logs` 用同一个 `LogsQuery` 但走 `Query` 提取器，即 `?tail=200`。

**`purge_tasks` 的提取器是 `Option<Json<PurgeRequest>>`** —— 完全不发 body 也能过。这是刻意的：清理动作不该强迫调用方构造一个空 JSON。

**`/tasks/purge` 用 POST 而非 DELETE 的原因**（源码注释原文）：这是个带副作用的批处理动作且要带参数；DELETE 无 body 的惯例会让 `retentionDays` 只能走 query string。

### 3.3 踩坑（实测）

`POST /sync/comic` 只发部分字段会返回 **422**。`Comic` 有 20 个必填字段，且 `ComicInSearch.updateAt` 是 `i64`（**不是** ISO 字符串）—— 用字符串会被 serde 拒绝。全量 payload 实测 200。

---

## 四、响应体格式

### 4.1 成功

一律 `Json<T>`，`#[serde(rename_all = "camelCase")]`。除下列特例外，前端读到的字段名都是 camelCase。

**显式 `#[serde(rename)]` 成 snake_case 的字段**（这些是**故意**的，不是遗漏）：

| 字段 | 出现位置 |
|---|---|
| `total_views` | `Comic` / `ComicInSearch` |
| `series_id` | 同上 |
| `comment_total` | 同上 |
| `related_list` | 同上 |
| `is_favorite` | 同上 |
| `is_aids` | 同上 |
| `line_number` | `LogEvent`（`events.rs`） |

> ⚠️ **破坏性提醒**：把这 7 个字段改成 camelCase 会让**每一个前端读取点静默失效**（TS 侧读不到属性时是 `undefined`，不报错、不崩、编译期也拦不住）。必须前后端**同一次提交**一起改。已登记为待办项（盘点 #4）。

### 4.2 失败

统一由 `src-server/src/api/error.rs` 的 `ApiError` 映射：

```
HTTP 400
{ "errTitle": "...", "errMessage": "..." }
```

复用桌面版的 `CommandError` 结构体，**前端错误处理逻辑不需要改**。源码注释明确写了：业务失败统一用 400，前端只关心 body 里的 `errTitle`/`errMessage`，不需要在 HTTP 状态码上做细分。

⚠️ 因此**不存在 404/409/500 的业务区分** —— 所有业务失败都是 400。只有 axum 自身的提取器失败（如 422 反序列化错误、405 方法不允许）才会走别的状态码。

---

## 五、三处已登记的「不一致 / 缺口」

以下三项**不是 bug 报告**，是**已知事实登记**。修复与否是产品决策，不在本次文档工作范围内。

### 5.1 `state` 字段大小写不一致（已在前端归一化）

| 来源 | 大小写 | 示例 |
|---|---|---|
| `GET /api/tasks`（REST，走 SQLite） | **lowercase** | `"pending"` / `"done"` / `"failed"` |
| WebSocket 事件（走内存） | **PascalCase** | `"Pending"` / `"Downloading"` / `"Done"` |

**归一点**：`src/panes/ProgressesPane/ProgressesPane.vue` 的 `taskViewToProgressData()` —— 它把 REST 的小写 state 转成 PascalCase 后再渲染。

**后果**：任何绕过 `taskViewToProgressData()` 直接消费 REST 响应的地方，都必须自己做大小写归一，否则状态显示会静默错位（找不到映射时通常回落到默认色/默认文案）。改动 REST 侧大小写 = 打破这个归一点，**必须同时改前端**。

### 5.2 9 个端点返回无类型的 `serde_json::json!`

| # | 端点 | 返回 |
|---|------|------|
| 1 | `GET /api/health` | `{"status":"ok"}` |
| 2 | `GET /api/auth/check` | `{"ok":true}` |
| 3 | `DELETE /api/tasks/:chapter_id` | `{"ok":true}` |
| 4 | `GET /api/catalog/status` | 见下方字段表 |
| 5 | `POST /api/catalog/trigger` | `{"running":bool,"result":?}` |
| 6 | `POST /api/server/info` | 见下方字段表 |

> **口径说明**：早期盘点记作「9 个」，是按 `json!(` 宏的**出现次数**数的（含 `commands.rs` 内 1 处、`routes.rs` 内 8 处）。按**端点数**是上表 6 条。两种口径都记录在此，避免后续对不上账。

**`GET /api/catalog/status` 的实际字段**（源码 `json!` 展开）：

```
attached: bool              // catalog 为 None 时 false，其余字段仍返回
path: string | null
autoFill: bool              // 来自 config.catalog_auto_fill
batch: number               // 来自 config.catalog_fill_batch
running: bool
rounds: number
markedDone: number
inFlight: number
counts: { [state]: number } | null
```

源码注释特意说明：库没挂进来时 `attached=false`，**其余字段仍返回**，前端不用为「未接入」单独写一套渲染分支。

**`POST /api/server/info` 的实际字段**（实测返回）：

```json
{"dataDir":"/data","downloadDir":"/data/漫画下载","eventSubscribers":3,"version":"0.1.0"}
```

**为什么登记为 legacy**：这些响应**没有 Rust 侧结构体**，因此 `bindings.ts` 里没有对应类型，前端读到的是 `any`。字段改名、增删都不会有任何编译期保护，也不会有任何测试失败。改这些端点前必须先手工 grep 前端所有读取点。

### 5.3 `/login` 与 `/user/profile` 是前端幽灵调用

| 事实 | 位置 |
|---|---|
| 前端**调用** `/login` | `src/bindings.ts:353` |
| 前端**调用** `/user/profile` | `src/AppContent.vue:63` |
| 后端**注册**这两个路由 | **没有**。`routes.rs:46` 的注释原文：`/server/info`、`/login`、`/user/profile` 「已随去前端一并删除」 |

**实际后果**：请求会落到 `ServeDir` fallback → **404**（不是 400，因为没进 axum handler）。

**注意这条注释已经过期**：`/server/info` 后来在 `c91d294^` 被**恢复**（`routes.rs:104`，仅 POST），但第 46 行的注释没同步更新，现在读起来是错的。

**未决问题（产品决策，不是技术债）**：这两个调用是删掉，还是在后端补上路由？在决定之前，它们会一直产生 404。

---

## 六、相关文件

| 文件 | 作用 |
|---|---|
| `src-server/src/api/routes.rs` | 路由表 + 全部 handler + 请求结构体（599 行） |
| `src-server/src/api/commands.rs` | 22 个业务函数，handler 的实际实现 |
| `src-server/src/api/error.rs` | `ApiError` / `api_try!` 宏 |
| `src-server/src/api/ws.rs` | WebSocket handler |
| `src-server/src/event_bus.rs` | 事件总线 + `topics` 常量 |
| `src/bindings.ts` | 前端调用面（tauri-specta 形状） |