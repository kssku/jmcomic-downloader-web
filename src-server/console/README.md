# 单文件控制台（归档）

这是项目早期的零依赖单文件控制台，通过 `include_str!` 编译进二进制。
后来前端改为 Vue 3 SPA（见 `docs/API.md` 和根 `README.md`），
`src-server/static/` 改为存放 Vue 构建产物，本文件保留作历史参考。

**当前不使用。** 如要恢复：改 `main.rs` 的 `ServeDir` 指向本目录，
并把本文件拷回 `src-server/static/`。