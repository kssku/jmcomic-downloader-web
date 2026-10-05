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
- [x] 部署验证

---

## 技术栈

- 后端：Rust、axum、SQLite、reqwest
- 前端：TypeScript + Vue 3 + Vite + UnoCSS + naive-ui + Pinia
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

## 安装

### 前置要求

- Docker 20.10+

### 快速开始

拉取镜像并启动：

```bash
docker run -d \
  --name jmcomic-server \
  -p 8080:8080 \
  -v jmcomic-data:/data \
  -e TZ=Asia/Shanghai \
  --restart unless-stopped \
  ghcr.io/kssku/jmcomic-downloader-web:latest
```

浏览器打开 `http://localhost:8080`。

### 数据持久化

容器内 `/data` 存放：

- SQLite 数据库（任务列表、下载记录）
- 配置文件（`config.json`）
- 日志

**必须挂载 volume**——否则容器删除后数据全丢。

下载产物默认落在 `/data/漫画下载`，**在 `/data` 之内**，因此挂载 `/data` 即可一并持久化，无需额外挂载。若想单独指定下载目录（例如接后处理流水线），再额外挂一个卷，并在网页「配置」里把下载目录改到该挂载点。

### 端口

容器内部固定监听 `8080`。宿主机端口自行映射：

```bash
-p 9080:8080    # 宿主机 9080 → 容器 8080
```

### 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `JM_DATA_DIR` | `/data` | 数据目录（数据库、配置、日志） |
| `JM_BIND` | `0.0.0.0` | 监听地址 |
| `JM_PORT` | `8080` | 容器内监听端口 |
| `JM_AUTH_DISABLED` | `true` | 认证开关。**默认关闭认证**（纯 API 后台场景）。设为 `false` / `0` 才开启令牌校验 |
| `JM_AUTH_TOKEN` | 随机生成 | 访问令牌。仅在认证开启时生效；不设置则随机生成并打印到日志 |
| `JM_AUTH_USER` | `admin` | Basic Auth 用户名 |
| `TZ` | `UTC` | 时区 |

> ⚠️ **认证默认是关闭的**：这意味着任何能访问到该端口的人都可以直接调用 API。
> 若部署在公网或不可信网络，**务必**设置 `-e JM_AUTH_DISABLED=false` 并配合
> `-e JM_AUTH_TOKEN=<你的令牌>`，否则等于裸奔。

### docker compose

创建 `docker-compose.yml`：

```yaml
services:
  jmcomic:
    image: ghcr.io/kssku/jmcomic-downloader-web:latest
    container_name: jmcomic-server
    ports:
      - "8080:8080"
    volumes:
      - ./data:/data
    environment:
      - TZ=Asia/Shanghai
      # 需要认证时打开下面两行
      # - JM_AUTH_DISABLED=false
      # - JM_AUTH_TOKEN=改成你自己的令牌
    restart: unless-stopped
```

启动：

```bash
docker compose up -d
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