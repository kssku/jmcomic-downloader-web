# 交接文档

## 项目

- 仓库：https://github.com/kssku/jmcomic-downloader-web
- 目标：把 pica 版下载器后端改造成 jm 数据源，前端保持 Web 架构。
- 策略：**路线 B** —— 保留 String 架构，只换 jm 数据源。

## 当前状态（提交见 git log）

- 阶段 1a：`jm_client.rs`、`responses/`、`context.rs`、`extensions.rs`、`types/comic.rs` 已换成 jm。
- **阶段 1b/1c：已完成，`cargo check` / `cargo test` 全过（42 个单测通过）。**

### 阶段 1b/1c 具体改动

- `types/get_favorite_sort.rs`：枚举改名为 `FavoriteSort`（jm 语义）。
- `types/get_favorite_result.rs` / `search_result.rs`：按 jm 结构重写（去 `ImageRespData`/`Pagination`）。
- `utils.rs`：`get_comic` 重写为 jm 单次 `get_comic(aid)`（无分页）；`filename_filter` 按桌面版补全全角映射；修掉被破坏的中文注释。
- `api/commands.rs`：`UserProfileDetailRespData` -> `GetUserProfileRespData`；`search_comic` 适配 `JmClient::search`（`SearchResp` 枚举，处理 redirect 命中单本）；`login` 改为 jm 语义（cookie 存 JmClient，返回用户名）。
- `api/routes.rs`：同步改类型。
- `download_manager.rs`（核心）：
  - `download_cover` 用 `Comic::get_cover_url()`（jm 封面 URL）。
  - `get_img_urls` 重写：一次 `get_chapter(id)` + `get_scramble_id(id)`，逐图算 `calculate_block_num`，返回 `Vec<(url, block_num)>`。
  - **block_num 编码进 url 的 fragment（`#block=N`）**，随图片清单落库，断点续传后仍可取回，无需改 DB schema。
  - `download_img` 解析 fragment 得 `block_num`，请求用去 fragment 的干净 url。
  - `save_img` 新增 `block_num` 参数，解码后按 `stitch_img` 还原（jm 图片纵向切块乱序）。
- `main.rs`：`pica_server` -> `jmcomic_server`。

### jm 图片还原算法（来自桌面版）

```
block_num:
  id < scramble_id            -> 0
  id < 268850                 -> 10
  else:
    x = id < 421926 ? 10 : 8
    block_num = (md5("{id}{filename}") 末字符 % x) * 2 + 2
```

`stitch_img`：把图按 block_num 纵向切块后逆序重排。

### 阶段 2：前端 jm 适配（本轮）

**后端**

- `types/comic.rs`：`Comic` 新增 `coverUrl` 字段（`#[serde(rename = "coverUrl", skip_deserializing, default)]`），
  由 `from_comic_resp_data` 与 `from_metadata` 显式填充（`get_cover_url()`）。前端详情页直接用，无需知道图床域名。
- `api/routes.rs`：`LoginRequest` 字段 `email` -> `username`（`#[serde(alias = "email")]` 兼容旧前端/脚本）；
  `login` handler 变量正名。jm 登录返回用户名，**不是**可当 Authorization 用的 token。

**前端**

- `src/bindings.ts`：类型全面对齐 jm——
  - 删 `Creator` / 旧 `Comic` / 旧 `ComicInSearch` / `Pagination` / `Image`。
  - 新 `Comic`（`name`/`author[]`/`tags[]`/`likes`/`totalViews`/`seriesId`/`chapterInfos`/`coverUrl`…）。
  - 新 `ComicInSearch`（`name`/`author`/`image`/`liked`/`isFavorite`/`updateAt`…）。
  - 新 `SearchResult`（`searchQuery`/`total`/`docs`，**无 limit/page/pages**）。
  - 新 `UserProfileDetailRespData`（`username`/`photo`/`fname`/`exp`/`levelName`…）。
  - `login(username, password)` 参数改名；去掉 `setToken`（jm 登录返回值不是 token）。
- `src/panes/SearchPane.vue`：解构改 `name`/`image`；分页按 `total` / `PAGE_SIZE(80)` 估算（jm 搜索不返回页数）。
- `src/components/ComicCard.vue`：props 改 `comicName`/`comicAuthor`/`image`；封面直接 `<img :src="image">`（jm 返回完整 URL）；去掉 `categories`。
- `src/panes/ChapterPane.vue`：详情区 `coverUrl` / `name` / `author.join('、')` / `tags.join('、')`。
- `src/panes/ProgressesPane/components/{Completed,Uncompleted}Progresses.vue`：`comic.title` -> `comic.name`。
- `src/dialogs/LoginDialog.vue`：`username` 参数；不再写 `store.config.token`；成功仅提示。
- `src/AppContent.vue`：头像 `userProfile.photo`（后端已拼完整 URL），用户名 `userProfile.username`。

**鉴权模型澄清（重要）**

后端 `auth.rs` 用独立的静态 `AuthConfig`（Bearer token / Basic），与 jm 登录**解耦**。
jm 登录只把 cookie 存进 `JmClient`，不产出可作 Authorization 的 token。
前端 `Authorization` 输入框 / `PICA_AUTH_DISABLED` 仍是唯一的 API 鉴权入口；
`LoginDialog` 现在只做 jm 账号登录，不再篡改 token。

**验证**

- `cargo check`：0 错误 0 警告；`cargo test`：42 passed。
- 前端 `vue-tsc` 未能在本机运行（无 `node_modules` 且无网络），改动经人工逐文件审查。

## 下一步

1. 端到端部署验证（jm 登录、搜索、点开详情、下载一章、断点续传）。
2. 真机跑一遍 jm 图片还原，确认 `calculate_block_num` 与 `stitch_img` 正确。
3. （可选）后端补收藏 / 周榜路由——`responses/get_favorite_*.rs`、`get_weekly_*.rs` 类型已在，
   但 `commands.rs` / `routes.rs` 尚未接线，前端也没有对应页面。
4. 前端 `vue-tsc` 类型检查需在装有 `node_modules` 的环境补跑。

## 本机参考

- jm 桌面版：`E:\github\jmcomic-downloader-main`（Tauri，Rust 后端在 `src-tauri`）
- 图片还原参考：桌面版 `src-tauri/src/download_manager.rs` 的 `calculate_block_num` / `stitch_img`

## 关键决策

1. 路线 B（保留 String 架构）。
2. 包名 `jmcomic-server`，数据目录 `JM_DATA_DIR`，db `jm_server.db`。
3. MIT，保留原项目版权。

