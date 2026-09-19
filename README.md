# jmcomic-downloader-web

禁漫天堂（jmcomic / 18comic）下载器 **Web / 服务端版**。

Rust 后端（axum + SQLite）+ 前端 SPA，可部署在服务器/NAS 的 Docker 容器中，
支持任务持久化、断点续传、失败重试，并可与青龙（QingLong）等后处理流水线对接：
下载 -> 打 CBZ -> 上传网盘归档。

> 本项目基于 `lanyeeee/jmcomic-downloader`（数据源逻辑）与哔咔 web 版服务端骨架改造而来。

---

## 状态

**开发中（WIP）**。当前进度见 [`docs/HANDOFF.md`](docs/HANDOFF.md)。

- [x] 阶段 1a：jm 数据层移植（jm_client / responses / 类型适配）
- [x] 阶段 1b/1c：数据模型 + 调度/命令层适配
- [x] 前端 UI 改 jm
- [ ] 部署验证

---

## 技术栈

- 后端：Rust、axum、SQLite、reqwest
- 前端：TypeScript + Vite
- 部署：Docker / docker-compose

---

## 构建

### 后端

```bash
cd src-server
cargo build --release
```

### 前端

```bash
pnpm install
pnpm build
```

### Docker

```bash
docker compose up -d --build
```

---

## 免责声明

本工具仅作学习、研究、交流使用。

- 使用者应自行承担使用本工具的全部风险。
- 作者不对使用本工具导致的任何损失、法律纠纷或其他后果负责。
- 作者不对使用者使用本工具的行为负责，包括但不限于使用者违反法律或任何第三方权益的行为。
- 请勿将本工具用于任何商业用途或非法用途。

---

## 许可

MIT License。本项目参考/移植了 `lanyeeee/jmcomic-downloader`（MIT）的部分逻辑，
相关版权归原作者所有，详见 [LICENSE](LICENSE)。