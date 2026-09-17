# HANDOFF - jmcomic-downloader-web

> 更新：2026-09-17 | 提交：`1a34c42` | 仓库：https://github.com/kssku/jmcomic-downloader-web（公开, MIT）

## 这是什么

禁漫天堂（jmcomic/18comic）下载器 **Web/服务端版**。Rust(axum)+SQLite 后端 + SPA 前端，Docker 部署，对接青龙流水线（下载->CBZ->115 归档）。

- 骨架来源：`picacomic-downloader-web`（哔咔版）
- 数据源来源：`lanyeeee/jmcomic-downloader`（桌面 Tauri 版，MIT）
- 本机参考路径：jm 桌面版 `E:\github\jmcomic-downloader-main`

## 核心策略（路线 B）

保留 pica 服务端的 **String 架构**（chapter_id: String、SQLite、路由、青龙、目录格式），**只换数据源为 jm**。不要照搬 jm 桌面版的 i64 types。

## jm 数据源要点

- 密钥：`18comicAPP` / `18comicAPPContent` / `185Hcomic3PAPP7R`，版本 `2.0.13`
- 签名：`token = md5(ts + secret)`，`tokenparam = "{ts},{version}"`
- 响应解密：AES-256-ECB，key=`md5(ts + APP_DATA_SECRET)`，输入 Base64
- 图片域：`cdn-msp2.jmapiproxy2.cc`；图片 URL：`https://{域}/media/photos/{chapter_id}/{filename}`；封面：`https://{域}/media/albums/{id}.jpg`
- 图片需 scramble 切块还原：`id<scramble_id`->0块；`scramble_id<=id<268850`->10块；`id>=268850`->`x=(id<421926?10:8)`,`block_num=(md5(id+filename)末字符%x)*2+2`；`stitch_img` 纵切逆序重排
- API 接口：`/login` `/search` `/album` `/chapter` `/chapter_view_template`(取 scramble_id) `/favorite` `/week`

## 已完成（阶段 1a, 提交 44e4f33）

`jm_client.rs`(432行)、`responses/`(9文件去specta)、`context.rs`(JmClient/JM_DATA_DIR/jm_server.db)、`extensions.rs`(get_jm_client/set_proxy)、`types/comic.rs`(jm字段,String id)、`types/mod.rs`(Category/CategorySub)、`utils.rs`(md5_hex)、`Cargo.toml`(aes/md5/cookies)

## 待办（阶段 1b/1c）—— 目标 cargo check 通过

### 1b 数据模型
- `types/favorite_sort.rs` -> jm `FavoriteSort{FavoriteTime,UpdateTime}`
- `types/search_sort.rs` -> jm `SearchSort{Latest,View,Picture,Like}`
- `types/get_favorite_result.rs` / `search_result.rs` -> 按 jm 结构重写（去 ImageRespData/Pagination）
- `types/mod.rs` 导出 `FavoriteSort`

### 1c 调度/命令层（核心）
- `utils.rs`：重写 `get_comic` —— jm 用 `get_comic(aid:i64)` 一次拉全（无分页）
- `api/commands.rs`：`UserProfileDetailRespData`->`GetUserProfileRespData`；`search_comic` 签名改 jm；String->i64 转换
- `api/routes.rs`：同上改名
- `download_manager.rs`（最重）：`comic.thumb`->`get_cover_url()`；`get_chapter_img`->`get_img_data_and_format(url)`+**scramble 还原**

### 已知问题
- 部分文件中文注释因 PowerShell 写入变乱码（逻辑不受影响，重写时修正）

## 编译状态

未通过（阶段 1c 待做，属预期）。错误集中在 download_manager/commands/utils 按 pica 字段访问 jm 结构。

## 后续（编译通过后）

前端 UI 改 jm（搜索/详情/收藏/周榜）、部署验证。README 免责声明已加。

## 关键决策

1. 路线 B（保留 String 架构）
2. 包名 `jmcomic-server`，数据目录 `JM_DATA_DIR`，db `jm_server.db`
3. MIT，保留原项目版权