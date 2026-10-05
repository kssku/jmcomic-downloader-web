# 数据库 Schema 参考

> 来源：`src-server/src/store/migrations.rs`（DDL）、`src-server/src/store/types.rs`（Rust 类型）、`src-server/src/catalog.rs`（外部候选池）。
> 本文件描述**当前代码事实**。

---

## 一、两条数据库，两套 schema

**这是最容易搞混的一点**：项目里有两个 SQLite 库，来源和归属完全不同。

| 库 | 文件 | 谁建的 | 谁在用 | 可写 |
|---|---|---|---|---|
| **任务库** | `JM_DATA_DIR/jmcomic_server.db` | 本项目（`store/migrations.rs` 的版本化迁移） | 下载任务状态持久化 | ✅ 读写 |
| **候选池** | `JM_DATABASES_PATH/jm.db` | **青龙侧**（外部系统） | 只读候选池补全 | ❌ **只读** |

`store/mod.rs` 的模块注释原文：

> **数据库文件独立**（`JM_DATA_DIR/jmcomic_server.db`），**不与青龙的 `bica_comics.db` 混用**，避免两套 schema 互相干扰。

`catalog.rs` 侧的对应约束：只 `UPDATE download_state`，**绝不碰 `comics` 表**（`mark_state` 的注释：「只更新 `state` 与 `updated_at`，绝不碰别的列」）。

---

## 二、任务库：`jmcomic_server.db`

### 2.1 为什么存在

`store/mod.rs` 的模块注释给出了两个要解决的根本问题：

1. **D1 任务状态纯内存** —— 容器一重启，所有 `Pending` / `Downloading` 任务凭空消失，用户不知道哪些下了哪些没下。
2. **D3 章节完整性判定过粗** —— 原先靠「已下载张数 == 总张数」判断章节是否成功，导致**单张图超时要整章重下**。有了图片级的 `download_image` 表，恢复时只需要重下 `state != 'done'` 的图片。

设计约束（同样来自模块注释）：

- **不引入外部组件**。NAS 上多一个 Redis / 消息队列就多一个故障点，而 SQLite 已经在青龙侧跑着，运维熟悉。
- **单写连接 + WAL**。章节并发 3 + 图片并发 20，写操作全部集中在状态迁移点，**不在图片下载热路径上逐张写盘**。
- **数据库文件独立**（见上节）。

### 2.2 版本管理

不用 `refinery` / `sqlx-migrate`。源码注释解释了原因：这个项目的迁移就两张表，引一套迁移框架的编译开销和心智负担都不划算。

机制：用 `PRAGMA user_version` 记录 schema 版本，逐版本向上迁移。

| 项 | 值 |
|---|---|
| `SCHEMA_VERSION` | **1** |
| 版本不符（DB 更新） | `anyhow::bail!` **拒绝启动**，提示「请升级 jmcomic-server，或备份后删除 `jmcomic_server.db` 重建」 |
| 为什么拒绝而不是继续 | 继续跑可能把数据写坏；直接静默失败又会让用户卡死。明确报错，由调用方决定 |

**每次改表结构都要 `SCHEMA_VERSION + 1` 并补一个 `migrate_vN` 函数** —— 源码注释里写死了这条规则。

### 2.3 连接级 pragma（`Store::tune`）

**必须在迁移之前执行。**

```sql
PRAGMA journal_mode = WAL;      -- 读不阻塞写
PRAGMA synchronous = NORMAL;    -- WAL 下的安全/性能平衡点
PRAGMA foreign_keys = ON;       -- download_image 的级联删除依赖它
PRAGMA busy_timeout = 5000;     -- 写冲突时等 5s 而不是立刻报错
```

⚠️ **`foreign_keys = ON` 是 `download_image` 级联删除的前提**。SQLite 默认**关闭**外键约束 —— 一旦这行 pragma 失效（比如某条代码路径绕过 `tune` 直接用连接），删 `download_task` 就不会级联删图片，留下孤儿行。

### 2.4 表：`download_task`（章节级任务）

主键：`chapter_id`。

| 列 | SQLite 类型 | 约束 / 默认 | Rust 类型 | 说明 |
|---|---|---|---|---|
| `chapter_id` | TEXT | **PRIMARY KEY** | `String` | |
| `comic_id` | TEXT | NOT NULL | `String` | |
| `comic_title` | TEXT | NOT NULL | `String` | |
| `chapter_title` | TEXT | NOT NULL | `String` | |
| `chapter_order` | INTEGER | NOT NULL DEFAULT 0 | `i64` | 章节序号 |
| `state` | TEXT | NOT NULL | `DbTaskState`（enum） | 见 §2.7 |
| `total_img_count` | INTEGER | NOT NULL DEFAULT 0 | `i64` | ⚠️ 见下方警告 |
| `done_img_count` | INTEGER | NOT NULL DEFAULT 0 | `i64` | ⚠️ 见下方警告 |
| `retry_count` | INTEGER | NOT NULL DEFAULT 0 | `i64` | |
| `last_error` | TEXT | （可空） | `Option<String>` | |
| `dir_fmt` | TEXT | NOT NULL DEFAULT `''` | `String` | 目录格式**快照** |
| `created_at` | INTEGER | NOT NULL | `i64` | Unix 秒 |
| `updated_at` | INTEGER | NOT NULL | `i64` | Unix 秒 |

**⚠️ 关于 `total_img_count` / `done_img_count`**（源码注释原文）：

> 保留是为了让前端继续用现有的 `downloaded/total` 渲染逻辑，但**完整性判定不再依赖它们**，而是查 `download_image` 里还有没有 `state != 'done'` 的行。见 `repo.rs` 的 `is_chapter_complete`。

**这是本表最重要的一个语义陷阱**：这两个计数是**给前端看的展示值**，不是**真相源**。任何用它们判断「这章下完了没」的新代码都会引入 D3 那个旧 bug（单张图失败导致整章重下）。

**⚠️ 关于 `dir_fmt`**（源码注释原文）：

> 目录格式快照。恢复时必须用任务自己当初的快照，而不是当前配置，否则用户中途改了 `dir_fmt`，恢复时就会找不到已下载的文件。

即：这是**写时快照**，不是配置引用。不要在恢复路径上改成读 `config`。

#### 索引

```sql
CREATE INDEX idx_task_state   ON download_task(state);
CREATE INDEX idx_task_comic   ON download_task(comic_id);
CREATE INDEX idx_task_updated ON download_task(updated_at);
```

`idx_task_updated` 的用途（源码注释）：**青龙侧按完成时间增量拉取**。对应 `GET /api/tasks?since=<unix秒>` 的增量游标参数（见 [`API.md`](./API.md)）。

### 2.5 表：`download_image`（图片级任务，断点续传核心）

主键：**复合** `(chapter_id, img_index)`。

| 列 | SQLite 类型 | 约束 / 默认 | Rust 类型 | 说明 |
|---|---|---|---|---|
| `chapter_id` | TEXT | NOT NULL，**FK → `download_task(chapter_id)` ON DELETE CASCADE** | `String` | |
| `img_index` | INTEGER | NOT NULL | `i64` | 章节内图片序号 |
| `url` | TEXT | NOT NULL | `String` | |
| `state` | TEXT | NOT NULL | `DbImageState`（enum） | 见 §2.7 |
| `retry_count` | INTEGER | NOT NULL DEFAULT 0 | `i64` | |
| `last_error` | TEXT | （可空） | `Option<String>` | |
| `bytes` | INTEGER | （可空） | `Option<i64>` | 下载字节数 |
| `updated_at` | INTEGER | NOT NULL | `i64` | Unix 秒 |

**外键级联**：删 `download_task` 一行会自动删掉该章节的全部 `download_image` 行。**依赖 §2.3 的 `PRAGMA foreign_keys = ON`。**

#### 索引

```sql
CREATE INDEX idx_image_pending ON download_image(chapter_id, state);
```

用途（源码注释）：**恢复时的高频查询：某章节还有哪些图没下完**。即 `WHERE chapter_id = ? AND state != 'done'`。

### 2.6 列顺序约定

`DbTask::from_row` 的注释：

> 列顺序必须与 `repo::TASK_COLUMNS` 一致。

即查询用的是**按位置取值**或按名取值，但 `TASK_COLUMNS` 这个常量是列顺序的**单一定义点**。改表结构时要同步改它。

### 2.7 字段名 → Rust 类型映射

#### `DbTaskState`（`download_task.state`）

| 字符串 | Rust 变体 |
|---|---|
| `"pending"` | `Pending` |
| （其余见 `types.rs`） | … |

`as_str()` / `parse()` 成对定义。

**⚠️ 与内存层刻意分开**（源码注释原文）：

> 与内存里的 `DownloadTaskState` 刻意分开：DB 层的状态是**跨进程的持久事实**，而内存层还要额外承载 `Downloading` 这种「本进程正在跑」的瞬态。两者转换见 `DownloadTaskState::to_db` / `from_db`。

**关键点**：`Downloading` **不落库**。容器重启后，一个原本 `Downloading` 的章节会从库里读成它上一次持久化的状态（通常是 `Pending`），而不是「正在下载」。任何新增的瞬态状态都应该走同一条路（只留在内存）。

#### `DbImageState`（`download_image.state`）

| 字符串 | Rust 变体 |
|---|---|
| `"pending"` | `Pending` |
| `"done"` | `Done` |
| `"failed"` | `Failed` |

`parse()` 对未知字符串返回 `anyhow::bail!("未知的图片状态 `{other}`")` —— **不是静默回落**。

### 2.8 损坏自愈（`Store::open_or_recover`）

打开失败时的处理流程：

1. `tracing::error!`（`err_title = "数据库损坏，将备份旧库并重建"`）；
2. 若 `db_path` 存在，改名成 `jmcomic_server.db.corrupt-<now_ts>`；
3. `tracing::warn!`（`err_title = "已备份损坏的数据库"`）；
4. **删掉 WAL 的两个附属文件** `-wal` / `-shm` —— 源码注释：否则重建的库会继承旧 WAL；
5. 重新 `Self::open(db_path)`。

⚠️ **第 2 步是 `rename` 不是删除** —— 旧库保住了。排查数据问题时先找 `*.db.corrupt-*` 文件，别以为数据没了。

### 2.9 时间戳

统一走 `store::now_ts()`（Unix 秒），**不各处调 `SystemTime::now()`**。

源码注释说明理由：方便日后换成注入式时钟做测试。

`catalog.rs` 的 `mark_state` 用的是**另一条路径** —— 交给 SQLite 自己算：

```sql
updated_at = CAST(strftime('%s','now') AS INTEGER)
```

源码注释：避免调用方传错格式。

---

## 三、候选池：`jm.db`（外部，只读）

**来源**：青龙侧建的表，本项目**只读 + 只回写 `download_state`**。

schema 定义**不在本仓库** —— `catalog.rs` 的测试 fixture 里有一份**只含用到的列**的近似版本（源码注释：「schema 与真实 `jm.db` 对齐（只含用到的列）」）：

```sql
CREATE TABLE comics (
    comic_id     TEXT PRIMARY KEY,
    title        TEXT,
    cover_status INTEGER DEFAULT 0
);

CREATE TABLE download_state (
    comic_id   TEXT PRIMARY KEY,
    state      TEXT NOT NULL DEFAULT 'pending',
    updated_at INTEGER DEFAULT 0
);
```

⚠️ **这是 fixture，不是真实 DDL。** 真实 `jm.db` 的 `comics` 表列数远多于 3 列（`catalog_status` 的注释提到全表聚合在 **88 万行**的库上约百毫秒级）。要拿准确 schema，必须直接读 NAS 上那个库。

### 3.1 本项目的写操作（唯一）

`JmCatalog::mark_state(comic_id, state) -> bool`：

```sql
UPDATE download_state
   SET state = ?1, updated_at = CAST(strftime('%s','now') AS INTEGER)
 WHERE comic_id = ?2
```

返回 `changed > 0`（即是否真的命中一行）。

**源码注释的硬约束**：**只更新 `state` 与 `updated_at`，绝不碰别的列。**

### 3.2 读操作

`JmCatalog::get_state(comic_id) -> Option<String>`：

```sql
SELECT state FROM download_state WHERE comic_id = ?1
```

`.ok()` 把 `QueryReturnedNoRows` 转成 `None` —— 用于回写前的核对与幂等判断。

另有 `state_counts()` 做全表聚合（`catalog_status` 用）。

---

## 四、并发与连接管理

`Store` 内部是 **`Arc<Mutex<Connection>>` 单写连接**。

| 方法 | 说明 |
|---|---|
| `with_conn(f)` | 借出连接。**源码注释：调用方持锁期间不要做网络 IO** |
| `with_tx(f)` | 在事务里执行。闭包返回 `Err` 时**自动回滚** |

`with_tx` 的实现：`conn.transaction()` → `f(&tx)?` → `tx.commit()`。注意 `?` 在 `commit` 之前 —— 闭包报错时 `tx` 被 drop，rusqlite 的 `Drop` 实现自动 rollback。

⚠️ **`with_conn` 的「持锁期间不要做网络 IO」是本层最重要的一条纪律**。图片下载是 20 并发，任何在持锁期间 await 网络的代码都会把整层串行化，表现为下载速度骤降到个位数。

---

## 五、破坏性提醒（改 schema 前必读）

1. **改表结构必须 `SCHEMA_VERSION + 1`**，否则老库不会被迁移，代码读新列直接报错。
2. **`total_img_count` / `done_img_count` 不是真相源**。任何完整性判定都必须查 `download_image.state != 'done'`。
3. **`Downloading` 不落库**。别在 DB 层加进程内瞬态状态。
4. **`foreign_keys = ON` 是级联删除的前提**。绕过 `tune` 用新连接 = 孤儿行。
5. **`dir_fmt` 是写时快照**。恢复路径上读 `config` 会让改了配置的用户找不到已下载文件。
6. **`catalog` 的 `comics` 表不属于本项目**。只读；任何写入都是跨系统污染。
7. **`store/mod.rs` 的「数据库文件独立」是硬约束**。把任务表塞进 `jm.db` 会与青龙的 schema 互相干扰。

---

## 六、相关文件

| 文件 | 作用 |
|---|---|
| `src-server/src/store/migrations.rs` | DDL + `SCHEMA_VERSION` + `migrate_v1` |
| `src-server/src/store/types.rs` | `Store` / `DbTask` / `DbImage` / 两个 state enum |
| `src-server/src/store/repo.rs` | `TaskRepo` / `ImageRepo` / `TaskConflict` / `TaskStats` / `is_chapter_complete` |
| `src-server/src/store/mod.rs` | 模块约束说明 + `now_ts()` |
| `src-server/src/catalog.rs` | `JmCatalog`：外部 `jm.db` 的只读访问 + `download_state` 回写 |