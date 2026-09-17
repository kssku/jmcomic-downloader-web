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

## 下一步

1. **前端 UI 改 jm**：搜索 / 详情 / 收藏 / 周榜的数据字段与文案。
2. 端到端部署验证（登录、搜索、下载一章、断点续传）。
3. 真机跑一遍 jm 图片还原，确认 `calculate_block_num` 与 `stitch_img` 正确。

## 本机参考

- jm 桌面版：`E:\github\jmcomic-downloader-main`（Tauri，Rust 后端在 `src-tauri`）
- 图片还原参考：桌面版 `src-tauri/src/download_manager.rs` 的 `calculate_block_num` / `stitch_img`

## 关键决策

1. 路线 B（保留 String 架构）。
2. 包名 `jmcomic-server`，数据目录 `JM_DATA_DIR`，db `jm_server.db`。
3. MIT，保留原项目版权。
