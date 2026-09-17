# HANDOFF - jmcomic-downloader-web 交接文档

> 最后更新：2026-09-17
> 当前提交：`44e4f33`（阶段 1a 完成，已 push）
> 仓库：https://github.com/kssku/jmcomic-downloader-web（公开）

---

## 一、项目简介

禁漫天堂（jmcomic / 18comic）下载器 **Web/服务端版**。
Rust 后端（axum）+ 前端 SPA，部署在 NAS 的 Docker 容器中，
对接青龙（QingLong）后处理流水线：下载 -> 打 CBZ -> 上传 115 网盘归档。

**来源**：
- 服务端骨架：从 `picacomic-downloader-web`（哔咔版）复制
- 数据源逻辑：参考 `lanyeeee/jmcomic-downloader`（桌面 Tauri 版，MIT）

---

## 二、改造策略（路线 B）

**保留** picacomic 服务端的 `String` 架构（chapter_id: String、SQLite、路由、青龙对接、目录格式），
**只把数据源换成 jm**。

理由：pica-server 架构已验证通过全部 6 条验收；jm 桌面版的 `types/` 为 Tauri 设计（i64 id），
照搬会引入大量不必要改动并破坏青龙对接。

---

## 三、jm 数据源要点（已移植到 jm_client.rs）

| 项 | 值 |
|---|---|
| 密钥 | `18comicAPP` / `18comicAPPContent` / `185Hcomic3PAPP7R` / 版本 `2.0.13` |
| 签名 | `token = md5(ts + secret)`；`tokenparam = "{ts},{version}"` |
| 响应解密 | AES-256-ECB，key = `md5(ts + APP_DATA_SECRET)`，输入 Base64 |
| 图片域 | `cdn-msp2.jmapiproxy2.cc` |
| 图片 URL | `https://{IMAGE_DOMAIN}/media/photos/{chapter_id}/{filename}` |
| 封面 URL | `https://{IMAGE_DOMAIN}/media/albums/{id}.jpg` |
| API 域（5 个） | `www.cdnzack.cc` / `www.cdnhth.cc` / `www.cdnhth.net` / `www.cdnbea.net` / `www.cdn-mspjmapiproxy.xyz` |
| 接口 | `/login` `/search` `/album` `/chapter` `/chapter_view_template` `/favorite` `/week` |
| 图片还原 | scramble 切块重排（见下） |

**scramble 还原算法**（jm 图片被切块乱序）：
- `id < scramble_id` -> 0 块（不切）
- `scramble_id <= id < 268850` -> 10 块
- `id >= 268850` -> `x = (id<421926 ? 10 : 8)`，`block_num = (md5(id+filename) 末字符 % x) * 2 + 2`
- `stitch_img`：纵切 block_num 块，逆序重排

---

## 四、已完成（阶段 1a，提交 44e4f33）

| 文件 | 内容 |
|---|---|
| `src-server/src/jm_client.rs` | 432 行：请求签名、AES 响应解密、登录/搜索/详情/章节/scramble_id/收藏接口、代理复用 |
| `src-server/src/responses/` | 9 个文件，全部换成 jm 结构（已去 specta） |
| `src-server/Cargo.toml` | 加 `aes` / `md5`；reqwest 加 `cookies` feature；包名 `jmcomic-server` |
| `src-server/src/context.rs` | `PicaClient` -> `JmClient`、`JM_DATA_DIR`、`jm_server.db` |
| `src-server/src/extensions.rs` | `get_jm_client`、`ClientBuilderExt::set_proxy`（含 System 代理修复） |
| `src-server/src/lib.rs` | 注册 `jm_client` |
| `src-server/src/types/mod.rs` | 加 `Category` / `CategorySub` |
| `src-server/src/utils.rs` | 加 `md5_hex` |
| `src-server/src/types/comic.rs` | 重写为 jm 字段（String id，name、series_id、author: Vec<String>） |

---

## 五、待办（阶段 1b/1c）—— 核心工作

> 目标：`cargo check` 通过，服务端能编译。

### 1b 数据模型适配
- [ ] `types/favorite_sort.rs`：改为 jm 的 `FavoriteSort { FavoriteTime, UpdateTime }`（已写好草稿）
- [ ] `types/search_sort.rs`：改为 jm 的 `SearchSort { Latest, View, Picture, Like }`（已写好草稿）
- [ ] `types/get_favorite_result.rs`：`GetFavoriteResult` / `ComicInFavorite` 按 jm 结构（id: String, name, image）重写
- [ ] `types/search_result.rs`：`SearchResult` / `ComicInSearch` 按 jm 结构重写
- [ ] `types/mod.rs`：导出 `FavoriteSort`（jm_client 引用 `crate::types::FavoriteSort`）
- [ ] `types/comic_info.rs`（可选）：jm 有，按需加

### 1c 调度/命令层适配
- [ ] `utils.rs`：**重写 `get_comic`** —— jm 用 `jm_client.get_comic(aid: i64)` 一次拉全（无分页）；`create_id_to_dir_map` 的 id 解析改 String（已是）
- [ ] `api/commands.rs`：
  - `get_user_profile` 返回类型 `UserProfileDetailRespData` -> `GetUserProfileRespData`
  - `search_comic(keyword, sort, page, categories)` -> jm 的 `search(keyword, page, sort, category, category_sub)`
  - `get_comic(comic_id: String)` 内部把 String 转 i64 调 jm
  - 引用 `ComicInSearch.name`（原 `title`）
- [ ] `api/routes.rs`：`UserProfileDetailRespData` -> `GetUserProfileRespData`
- [ ] `download_manager.rs`（**最重**）：
  - `comic.thumb` -> `comic.get_cover_url()`（3 处）
  - `get_chapter_img` -> `jm_client.get_img_data_and_format(url)` + **scramble 还原**
  - `Comic` 构造处 `title` -> `name`
- [ ] `jm_client.rs`：补 `search` 的调用方对齐；如需可加便捷方法 `get_comic_by_str`

### 已知问题
- [ ] **编码损坏**：部分用 PowerShell 写入的文件中文注释变乱码（`utils.rs`、`commands.rs` 等）；逻辑不受影响，需以正确 UTF-8 重写
- [ ] `types/get_favorite_result.rs` / `search_result.rs` 仍 import pica 的 `ImageRespData` / `Pagination`

---

## 六、参考路径（本机）

| 用途 | 路径 |
|---|---|
| jm 桌面版源码（参考） | `E:\github\jmcomic-downloader-main` |
| 本项目（web 版） | `E:\github\jmcomic-downloader-web` |
| 哔咔 web 版（模板） | 见 picacomic-downloader-web |

---

## 七、关键决策记录

1. **路线 B**：保留 String 架构，只换数据源（避免破坏已验证的青龙对接）
2. **包名**：`jmcomic-server`
3. **数据目录**：`JM_DATA_DIR`，db 文件 `jm_server.db`
4. **许可**：MIT，保留原项目版权声明（README 待加免责声明）

---

## 八、下一步建议

1. 按「阶段 1b -> 1c」顺序推进，每步 `cargo check`
2. 优先修 `utils.rs`（get_comic）与 `commands.rs`（签名），再啃 `download_manager.rs`（scramble）
3. 全部编译通过后：前端 UI 改 jm（搜索/详情/收藏/周榜）、README 免责声明、部署验证

---

## 九、免责声明（README 待补）

本工具仅作学习、研究、交流使用。使用者应自行承担风险。作者不对使用本工具导致的任何损失、法律纠纷或其他后果负责。