# 交接文档 — jmcomic-downloader-web

> 生成时间：2026-10-04
> 工作目录：`/root/GitHub/jmcomic-downloader-web`
> 交接时的 HEAD：`ad5f9aa`（已推送 origin/main），**工作区干净**

> **本文档的漂移状态已解除（2026-10-04 更新）**：初稿写成时 HEAD 是 `69d28e0`，
> 且 `index.html` 有 40 行未提交改动、测试只在 `/tmp`。这些现在都已在 git 里。
> 下方标 **[已处理]** 的小节保留原始记录，便于理解来龙去脉；标 **[现状]** 的是接手时要看的。
>
> **2026-10-04 后续更新**：`69d28e0` 的错误作者 `root <root@mx>` **已被重写为 `kssku`**（force push，
> 内容零改动，仅换 author 字段）。`69d28e0` 起的 6 条提交 hash 全部更新，映射见 §六。

---

## 一、当前状态速览

| 项 | 状态 |
|---|---|
| 本地 HEAD | `ad5f9aa` feat(console): 批量删除 + 配置标签补全 + 轮询去重 + 计数口径消歧 |
| 与远端 | 同步，无领先无落后 |
| 工作区 | **干净**（无未提交改动） |
| 容器 | `jmcomic-server` Up (healthy)，`0.0.0.0:8081->8080` |
| 镜像 | `jmcomic-server:api-only`（108MB）|
| 局域网 | http://192.168.31.154:8081 |
| 认证 | **关闭**（`JM_AUTH_DISABLED=true`，仅测试用）|

**关键点**：原先「镜像比 git 新」的漂移**已消除**。那 40 行已在 `de3c825`（原 `135b482`）提交并推送，
NAS 现在 `git pull` 能拿到了。

**提交序列**（全部作者 `kssku`，已推 origin/main）：

| 提交 | 内容 |
|---|---|
| `ad5f9aa` | 批量删除 + 配置标签补全 + 轮询去重 + 计数口径消歧 |
| `49dc478` | 修 5 处控制台缺陷并补注入回归测试 |
| `73a2e1a` | 更新交接文档至当前状态 |
| `9d6c085` | 前端测试入库（30 项 + 变异测试）|
| `de3c825` | 那 40 行：快照不覆盖分页 + 失败态区分空态 + 请求竞态防护 |
| `cb3d0b5` | 接上并发度热更新 + 修复 DB-only 任务操作报错 |

> **hash 变更说明**：`ad5f9aa` 之前的链是 `a98f720 → 5140d1d → c6cd2d0 → 9a8d237 → 135b482 → 69d28e0`。
> 2026-10-04 重写了 `69d28e0` 的作者（`root <root@mx>` → `kssku`），其后 5 条连带改写 hash，
> **文件内容零改动**（6 个 tree hash 逐个比对完全相同）。旧链备份在本地分支 `backup-before-rewrite-a98f720`。

> **镜像注意**：`de3c825`（原 `135b482`）改的是 `src-server/static/index.html`，它被 `ServeDir` 在**运行期**托管（COPY 进镜像）。
> 正在跑的容器镜像可能仍是改动前的旧前端。要验证那三个修复，需 `docker build` 重建，重启容器没用。

---

## 二、那 40 行是什么（**[已处理]**，已在 `de3c825`（原 `135b482`）提交）

文件：`src-server/static/index.html`

### 改动 1：WS 快照不再覆盖分页（P0 bug 修复）

**背景**：`task-snapshot-event` 由后端在 WS 连接建立时发一次（`ws.rs:52-60`），内容是**所有未完结任务的全量列表**，排序是「内存任务优先 + DB `created_at ASC`」。

但任务列表走的是分页 API：`updated_at DESC` + `state`/`comicId` 筛选，每页 50 条。**两套语义不同。**

**修复前的代码**（`index.html:1235-1238`）：

```js
if (topic === 'task-snapshot-event' && Array.isArray(payload)) {
  state.tasks = payload;    // ← 全量覆盖，无视分页/筛选
  renderTasks();
  return;
}
```

**实测复现的后果**（造了 60 条任务）：

| 路径 | 返回条数 |
|---|---|
| `GET /api/tasks?limit=50&offset=0` | 50 |
| `GET /api/tasks?limit=50&offset=50` | 10 |
| **WS 快照** | **60（全量）** |

用户翻到第 2 页 → WS 重连或刷新 → 列表突然变 60 行，页码却还显示「第 2 页」。**筛选同样被冲掉。**

**修复后**：快照只当「有变化」的信号，重新拉当前页：

```js
loadTasks();   // 分页/排序/筛选的唯一真相源始终是 API
```

### 改动 2：`loadTasks()` 加请求序号防竞态

`loadTasks` 会被筛选、翻页、WS 事件**并发触发**。慢响应后到会覆盖新数据。

```js
const seq = ++state.loadSeq;
const r = await api('GET', `/api/tasks?${buildQuery()}`);
if (seq !== state.loadSeq) return;   // 过期响应，丢弃
```

`state.loadSeq` 是新增字段（`index.html:717-719`）。

### 改动 3：加载失败区分于空态（P0 bug 修复）

**修复前**：`loadTasks` 的 `catch` 只 `log()`，`state.tasks` 保持旧值或空 → 界面停在「📭 没有匹配的任务」。**用户分不清「真的没有」和「请求挂了」。**

**修复后**：清空 + 显式错误态，新增 `renderTasksError(message)`：

```js
} catch (e) {
  if (seq !== state.loadSeq) return;
  log(`加载任务失败：${e.message}`, 'err');
  state.tasks = [];
  state.totalTasks = 0;
  renderTasksError(e.message);
}
```

顺带删掉了 `r.tasks || r.items || []` 里的 `r.items`——后端契约是 `tasks`，`items` 是死代码。

---

## 三、验证证据（已做过，可复现）

### 前端测试（**[现状]** 已入库，不再易失）

测试在 `src-server/tests/frontend/`，**共 30 项**，跑法：

```bash
pnpm test:console            # 30 项，预期全过
pnpm test:console:mutation   # 变异测试，预期 exit 0
```

harness **每次运行都从 `index.html` 现抽 `<script>` 块执行**，不依赖复制副本——
副本会随源码改动失效，那样测的就是旧代码。

| 文件 | 项数 | 覆盖 |
|---|---|---|
| `test-behavior.cjs` | 18 | 分页、筛选、失败区分空态、竞态 `loadSeq`、就地更新 |
| `test-ws.cjs` | 12 | 走**真实 `ws.onmessage` 分支**的 WS 快照行为 |
| `mutation-check.sh` | — | 把 `onmessage` 里的 `loadTasks()` 换回 `state.tasks = payload`，期望测试变红 |

覆盖的场景：

| 分组 | 断言 |
|---|---|
| P0-1 分页 | 第 2 页仍请求 `offset=50`；列表仍 10 条未被撑成 60 |
| P0-1c 筛选 | 快照后仍带 `state=failed`；筛选未被冲成全量 |
| P0-1d 非数组 | 非数组 payload 不触发加载 |
| P0-2 失败态 | 显示「加载失败」+ 后端错误原文；不误显示空态文案 |
| P0-2b 空态 | 空结果**不**报错；`total` 正确归零 |
| 竞态 | 慢响应回来后**被丢弃**，不覆盖新数据 |
| 就地更新 | 同页任务状态/进度/百分比正确更新 |

> **测试缺口已闭合**（原缺口说明见下）：初稿时 P0-1 组测的是「再调一次 `loadTasks()`」，
> **没走 WS 的 `onmessage` 分支**，属间接验证。现在 `test-ws.cjs` 通过打桩 `WebSocket`
> 捕获真实的 `onmessage` 并投喂快照 payload，**直接**覆盖该分支。

#### 变异测试：证明这测试真能抓到回归

一个全绿的测试**不能证明它有效**——它必须在缺陷复现时变红。

`mutation-check.sh` 把 `onmessage` 里的 `loadTasks()` 换回修复前的 `state.tasks = payload`，期望：

- **exit 1**（干净失败，非崩溃）→ 测试有效
- **exit 0**（仍全绿）→ 测试无效，抓不到缺陷

实测结果：`7 passed, 5 failed`，exit 1，未崩溃。且它抓到**两个维度**——
既抓到「没重新加载」（`calls=0`），也抓到**真实的数据损坏**（60 条快照顶掉 10 条、筛选被冲成 60）。
脚本用 `trap` 保证无论成败都还原源码。

### 后端测试（上一提交）

```
cargo clippy --all-targets -- -D warnings   → CLIPPY_EXIT=0
cargo test --lib                            → TEST_EXIT=0, 59 passed
```

### 运行期验证（上一提交的两个修复）

| 操作 | 修复前 | 修复后 |
|---|---|---|
| DB-only 任务 `pause` | 400 未找到 | **200**，DB → `paused` |
| 重复 `pause`（幂等重放） | 400 | **200** |
| DB-only 任务 `resume`/`cancel` | 400 | **200** |
| 真不存在的任务 | 400 | **400**（仍正确报错）|
| 并发度热更新 | 无日志、不生效 | **两条 INFO**，7/33 生效 |

日志现场（容器日志里能看到）：

```
download_manager.rs:224  下载并发度已就地更新  chapter_concurrency=7 img_concurrency=33
api/commands.rs:67       下载并发度已更新      chapter_concurrency=7 img_concurrency=33
```

---

## 四、环境搭建记录（Rust 工具链）

本机原先**没有 Rust**，本次装的：

| 项 | 值 |
|---|---|
| 安装方式 | rustup，系统级，`--no-modify-path` |
| `RUSTUP_HOME` | `/usr/local/rustup` |
| `CARGO_HOME` | `/usr/local/cargo` |
| 工具链 | `stable-x86_64-unknown-linux-gnu` |
| rustc / cargo | **1.99.0** |
| 附加组件 | clippy 0.1.99、rustfmt 1.10.0-stable、rust-docs |
| PATH 配置 | `/etc/profile.d/rust.sh` |
| 镜像源 | rsproxy 稀疏索引 → `/usr/local/cargo/config.toml` |

验证（干净环境，不依赖手动 source）：

```bash
env -i HOME=/root bash -lc 'which cargo && cargo --version'
# /usr/local/cargo/bin/cargo
# cargo 1.99.0
```

### 网络路由：必须记住的两条相反结论

| 目标 | 直连 | 走代理 `192.168.31.124:7890` |
|---|---|---|
| `crates.io` | **TLS 掐断** | 可用 |
| `rsproxy.cn` | **0.13 秒，可用** | — |
| `github.com` | **0.69 秒，可用** | **TLS 掐断** |

**同一个代理对不同目标行为不一致。遇到 TLS 掐断，两边都试一遍，不要一概而论。**

`/usr/local/cargo/config.toml` 用的是 **sparse 协议**（HTTPS 单次请求），这是绕开 TLS 长连接问题的关键，同时设了 `git-fetch-with-cli = true` 兜底。

### 残留可清理项

| 路径 | 大小 | 说明 |
|---|---|---|
| `/root/.cache/dsh-cargo` | 514M | Docker 构建用的 cargo 缓存 |
| `/usr/local/cargo/registry` | 514M | 本机 cargo 依赖缓存 |

两者内容重叠，**建议保留**——后续编译不用重下 294 个依赖。

---

## 五、Docker 构建与运行

### 镜像

```bash
cd /root/GitHub/jmcomic-downloader-web
docker build -t jmcomic-server:api-only -f Dockerfile .
```

- 期望镜像名/tag 与 `docker-compose.yml` 一致：`jmcomic-server:api-only`
- 端口：`8081 -> 8080`
- 镜像大小：**108MB**
- **27 步全过，BUILD_EXIT=0**

### Dockerfile 的一个关键设计（别误判为卡住）

`server` 阶段用**预编译技巧**：先只拷 `Cargo.toml` + `Cargo.lock`，用空 `main.rs` 骗 cargo 把 294 个依赖编成缓存层，再拷真实源码增量编译。

**在 `cargo build --release` 那步去看，`Build Cache` 会显示 0B**——因为该层缓存要等指令结束才落盘。这是正常现象，**不是卡住**。

### 静态托管相关的 Dockerfile 改动（2026-10-04）

从「编译期嵌入」改为「运行期 `ServeDir` 托管」时，Dockerfile 有四处改动：

1. **runtime 阶段新增** `COPY src-server/static /app/static` —— 静态目录必须真实存在于镜像里。
2. **紧跟 `RUN find` 固定权限位**，而不是 `chmod -R`：
   ```dockerfile
   RUN find /app/static -type d -exec chmod 755 {} + \
       && find /app/static -type f -exec chmod 644 {} +
   ```
   **为什么必须分开**：`chmod -R 644` 会把**目录也设成 644**，目录缺 x 位就进不去；`chmod -R 755` 会把文件也变成可执行。文件和目录权限语义不同，必须分开设。

   **为什么必须显式固定**：宿主机若启用特殊 ACL（飞牛 fnOS 的存储池），源文件权限位可能是 `000`。Docker 的 `COPY` 原样保留权限位，`chown` 只改归属不改权限，所以必须补 `chmod`。
3. **移除 `JM_STATIC_DIR` 环境变量** —— 它是纯 API 模式时代的占位符，Rust 源码从不读它。
4. **移除 `/app/empty-static` 占位目录** —— 不再需要「假装静态目录存在」。

**回归验证**：镜像内 `/app/static` 应为 `755 jm:jm`，`index.html` 为 `644 jm:jm`（已实测确认）。

### 前端是 `ServeDir` 运行期托管 `static/` 目录的

`main.rs`：`api.fallback_service(ServeDir::new("static"))`

> **2026-10-04 变更**：不再是 `Html(include_str!("../static/index.html"))` 编译期嵌入。
> 改为运行期读 `static/` 目录，`tower-http` 启用 `fs` feature。

**不挂任何 SPA fallback。** `ServeDir::new()` 的 `fallback` 默认是 `None`，文件存在就返回，不存在直接 404。曾出现的「`/favicon.png` 返回 index.html」根因是外层多挂了一层 SPA fallback 拦截了 ServeDir 的 404 —— 只要不挂 fallback，该 bug 在结构上不可能发生。

**改了 `index.html` 仍须重新 `docker build`**（因为静态资源是 COPY 进镜像的，不是挂载的），光重启容器没用。Dockerfile 用 `COPY src-server/static /app/static` + `find/chmod` 固定权限位。

> 注意：Dockerfile 里 **web 阶段（`pnpm exec vite build`）是注释掉的**，这是**有意为之**——项目有两条前端路线，后端自带的单文件控制台（当前工作区 64452 字节，零外部依赖）已够用。**不要误以为「没有前端」。**

### `.env`（测试专用，未提交，已被 gitignore 忽略）

```ini
JM_HOST_PORT=8081
JM_AUTH_DISABLED=true          # ← 安全风险：局域网无认证
JM_AUTH_USER=admin
JM_AUTH_TOKEN=***              # 已脱敏
JM_DATA_PATH=/srv/jmtest/data
JM_COMIC_PATH=/srv/jmtest/comic-download
JM_DATABASES_PATH=/srv/jmtest/databases
JM_HTTP_PROXY=http://host.docker.internal:7890
JM_HTTPS_PROXY=http://host.docker.internal:7890
JM_NO_PROXY=localhost,127.0.0.1,::1,www.cdnhth.cc,*.jmapiproxy2.cc,*.jmapiproxy.cc,*.cdnhth.cc
```

**数据路径为什么在 `/srv` 而不是 `/root`**：容器内进程以 uid 1000 运行，而 **`/root` 是 `drwx------`——uid 1000 连进都进不去**。这跟子目录属主无关，`chown` 子目录没用。必须放在 `/root` 之外。

已在 `.gitignore:19` 和 `.dockerignore:22-23` 里排除，**不会进仓库**。

---

## 六、已知问题与缺口

### 高优先级

1. ~~那 40 行前端改动未提交~~ —— **[已处理]** 已在 `de3c825`（原 `135b482`）提交并推送。
2. **认证关闭** —— 局域网内任何人可访问、改配置、触发下载。测完应 `docker compose stop` 或改回 `JM_AUTH_DISABLED=false`。
3. ~~测试没有真正走 WS `onmessage`~~ —— **[已处理]** 见 §3，`test-ws.cjs` 已直接覆盖该分支并有变异测试佐证。
4. ~~`69d28e0` 的作者身份是 `root <root@mx>`~~ —— **[已处理]** 已 force push 重写为 `kssku`（新 hash `cb3d0b5`）。
   根因：本机 `$HOME` 未设置，git 读不到全局 config，回退成 `whoami@hostname`。
   已在仓库级设好 `user.name=kssku`（`git config user.name` 可确认），后续提交不会再错。
   **重写前核实的四项事实**：`forks_count: 0`、贡献者仅 `kssku`、提交距今不到 24 小时、无外部协作迹象——
   「有人已拉取会冲突」的前提不成立。重写后 6 个 tree hash 逐个比对完全相同，**内容零改动**。
   远端已用 `git fetch --force` + GitHub API 双重确认无 `root` 残留。

### 中低优先级（本轮**决定不修**，理由记录在此）

| # | 问题 | 为什么不修 |
|---|---|---|
| P1-3 | `applyTaskEvent` 的 `Update` 分支在任务不在当前页时静默失效（`state.tasks` 只有当前页 50 条，`find` 返回 undefined 就什么都不做）| **`Update` 是图片级高频事件**，`emit_download_task_update_event()` 至少 11 处调用点（含图片进度写入）。「找不到就 `loadTasks()`」会每个图片进度打一次 API——把 P1 的小毛病换成 P0 的性能灾难。要做需先想清节流窗口设在哪一层 |
| P2-6 | `log()` + `toast()` 成对调用遍布 12+ 处 | 纯重构，价值不如风险 |
| P2-7 | `esc()` 用在 `onclick` 属性里是上下文错配（HTML 实体转义 ≠ JS 字符串转义）| 当前数据是后端生成的 `ch-001` 类 ID，**无可利用性**。正规做法是事件委托 + `data-` 属性 |
| P2-8 | 15 秒轮询与 WS 职责重叠 | 影响很小，WS 断线时轮询是必要的保底 |
| P2-9 | `Array.isArray(payload)` 类型判断在协议改为 `{tasks, total}` 时会静默跳过 | 需要服务端分页才会触发，届时一并改 |

### 前端体检报告里确认**没问题**的部分

避免接手者过度担心：

- **XSS 防护基本到位**：`esc()` 覆盖 `&<>"'`，6 处 `innerHTML` 逐个查过，只有上面 P2-7 那一处是上下文错配
- **WS 重连完整**：指数退避 `2^n` 封顶 15 秒，`onopen` 重置计数
- **日志不会无限增长**：400 行上限，`removeChild(firstChild)`
- **Toast 有清理**，不泄漏 DOM
- **删除有二次确认**
- **`aria-*` 用得认真**：`role="progressbar"` + `aria-valuenow`、`role="log"` + `aria-live`、`role="status"`

---

## 七、接手后建议的第一步

> 原稿写的「先 `git diff` 决定那 40 行去留」**已不适用**——它们已在 `de3c825`（原 `135b482`）。
> 下面是当前的接手动作。

```bash
cd /root/GitHub/jmcomic-downloader-web

# 1. 确认工作区干净、与远端同步
git status && git log --oneline -3

# 2. 前端测试（已入库，不再依赖 /tmp）
pnpm test:console            # 预期 30 passed
pnpm test:console:mutation   # 预期 exit 0（证明测试有效）

# 3. 后端基线复验（工具链已装好，登录 shell 直接用）
cd src-server && cargo clippy --all-targets -- -D warnings && cargo test --lib

# 4. 若要验证那三个前端修复在真实容器里生效，必须重建镜像
#    前端资源是 COPY 进镜像的（ServeDir 运行期读 static/），重启容器没用
cd .. && docker build -t jmcomic-server:api-only -f Dockerfile .
```

---

## 八、NAS 生产环境

**从 Dell 不可达**（`192.168.31.124:8081` ping 不通）。以下操作需在 NAS 侧执行：

```bash
# NAS 上，代码路径 /vol1/1000/apps/jmcomic-downloader-web
git pull origin main
```

GitHub 上现在有 `cb3d0b5`（原 `69d28e0`，作者已重写），包含：

- `cb3d0b5` 后端两个修复（并发度热更新接线 + DB-only 任务操作）
- `de3c825` **那 40 行前端改动**（快照不覆盖分页 + 失败态区分空态 + 请求竞态防护）
- `9d6c085` 前端测试入库（30 项）
- `ad5f9aa` 控制台批量删除 + 配置标签 + 轮询去重 + 计数消歧

**NAS `git pull` 现在能全部拿到**（原稿写的「40 行还没推、NAS 拉不到」已不适用）。

> 注意：`de3c825`（原 `135b482`）改的 `index.html` 是编译期嵌进二进制的。NAS 拉完代码**还需要重建镜像**才会在容器里生效。

---

## 九、关键文件索引

| 文件 | 作用 |
|---|---|
| `src-server/static/index.html` | 单文件控制台（`ServeDir` 运行期托管，COPY 进镜像）|
| `src-server/src/api/commands.rs:35-43` | `save_config` 的差量检测（含新增 `concurrency_changed`）|
| `src-server/src/api/commands.rs:59-67` | 并发度更新日志 |
| `src-server/src/context.rs:205` | `apply_concurrency`（原名 `reload_download_manager`）|
| `src-server/src/download_manager.rs:319-378` | `set_task_state`（DB 优先，内存降级）|
| `src-server/src/download_manager.rs:395` | `task_snapshot`（WS 快照来源）|
| `src-server/src/api/ws.rs:48-60` | 快照发送时机（连接建立时一次）|
| `src-server/src/jm_client.rs:399` | RwLock 读锁跨 await 的修复点 |
| `Dockerfile:34-70` | web 阶段（注释掉，有意为之）|
| `Dockerfile:116-118` | `static` 目录拷贝 |
| `docker-compose.yml` | 期望镜像 `jmcomic-server:api-only`，端口 8081→8080 |

---

## 十、一句话总结

**后端两个真 bug 已修、已验证、已推送**（并发度热更新从未接线、DB-only 任务操作报错）。**前端三个改动已提交并推送**（`de3c825`，原 `135b482`），原先「镜像比 git 新」的漂移**已消除**。

测试基建方面：后端有 59 个测试 + clippy 干净；前端测试**已入库** `src-server/tests/frontend/`，共 37 项（18 行为 + 12 WS + 7 注入）+ 一个变异测试，不依赖 `/tmp`、不依赖网络与数据库。

**接手时最该知道的一件事**：`de3c825`（原 `135b482`）改的是 `ServeDir` 运行期托管的 `index.html`。
正在跑的容器镜像**可能仍是旧前端**——要验证那三个修复，必须 `docker build` 重建，重启容器无效。

**仍开放的事项**（有意未动，见 §6）：

1. ~~`69d28e0` 的作者是 `root <root@mx>`~~ —— **已重写为 `kssku`**（2026-10-04 force push，内容零改动）。新 hash 见 §六。
2. 认证仍关闭（`JM_AUTH_DISABLED=true`），测完应关掉
3. ~~`applyTaskEvent` 的 `Update` 分支在高频事件下静默失效~~ —— **已修**（`1225c5c7`，即原 `5140d1d`）