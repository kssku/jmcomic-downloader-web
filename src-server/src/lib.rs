//! jmcomic-downloader Web 服务端
//!
//! 该 crate 以 picacomic-downloader-web 的服务端为模板，把数据源换成 jmcomic，改造成一个独立的
//! HTTP + WebSocket 服务，方便在 NAS 上以 Docker 方式部署，通过网页后台控制。

pub mod api;
pub mod auth;
pub mod config;
pub mod context;
pub mod download_manager;
pub mod errors;
pub mod event_bus;
pub mod events;
pub mod extensions;
pub mod logger;
pub mod jm_client;
pub mod responses;
pub mod store;
pub mod types;
pub mod utils;