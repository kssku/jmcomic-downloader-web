//! 用 `AppContext` 替代 Tauri 的 `AppHandle`。
//!
//! 原桌面版通过 `app.state::<T>()` / `app.path().app_data_dir()` / `event.emit(&app)`
//! 访问全局资源。Web 版把这些收敛到一个显式、可克隆的上下文对象里，
//! 从而让 `jm_client` / `download_manager` / `types` 等核心模块完全脱离 Tauri。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use parking_lot::RwLock;

use crate::catalog::JmCatalog;
use crate::catalog_sync::{CatalogSyncState, SharedCatalogSyncState};
use crate::config::Config;
use crate::download_manager::DownloadManager;
use crate::event_bus::EventBus;
use crate::jm_client::JmClient;
use crate::store::Store;

/// 运行期路径。全部来自环境变量，Docker 里挂一个卷到 `/data` 即可。
#[derive(Debug, Clone)]
pub struct Paths {
    /// 数据根目录（配置、日志、默认下载目录都在它下面）
    pub data_dir: PathBuf,
}

impl Paths {
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = std::env::var("JM_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));
        std::fs::create_dir_all(&data_dir)
            .with_context(|| format!("创建数据目录 `{}` 失败", data_dir.display()))?;
        Ok(Self { data_dir })
    }

    pub fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("日志")
    }

    /// 下载任务持久化数据库。与青龙的 `bica_comics.db` 完全独立。
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("jmcomic_server.db")
    }

    /// jm 候选池数据库（爬虫建的 88 万本元数据索引）。
    ///
    /// 默认 `/databases/jm.db` —— 这是三端下载器（jm / pica / wnacg）
    /// 共用的「漫画元数据」卷在容器内的挂载点。用 `JM_CATALOG_DB` 可覆盖。
    pub fn catalog_db_path(&self) -> PathBuf {
        std::env::var("JM_CATALOG_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/databases/jm.db"))
    }
}

/// 全局应用上下文。廉价可克隆（内部全是 `Arc`）。
#[derive(Clone)]
pub struct AppContext {
    paths: Paths,
    config: Arc<RwLock<Config>>,
    jm_client: Arc<RwLock<Option<JmClient>>>,
    download_manager: Arc<RwLock<Option<DownloadManager>>>,
    /// 任务持久化。构造阶段就打开——它没有循环依赖，不像
    /// `PicaClient` / `DownloadManager` 那样需要两段式构造。
    store: Store,
    /// jm 候选池（`/databases/jm.db`）。**可选**：库没挂进来、损坏、
    /// 无权限时为 `None`，此时下载器的既有功能照常可用，只是不自动补全。
    catalog: Option<JmCatalog>,
    catalog_sync: SharedCatalogSyncState,
    events: EventBus,
}

impl AppContext {
    /// 第一阶段构造：只加载配置与路径。
    /// `PicaClient` / `DownloadManager` 在之后通过 `init_runtime` 注入，
    /// 因为它们自身持有 `AppContext`，会形成循环引用。
    pub fn new(paths: Paths) -> anyhow::Result<Self> {
        let config = Config::load(&paths.config_path())?;

        // 数据库损坏时 `open_or_recover` 会备份旧库并重建空库，
        // 而不是让整个服务起不来。
        let store = Store::open_or_recover(&paths.db_path())?;

        // 候选池打不开**不阻塞启动**——它是增强功能，不是核心依赖。
        // 失败只记日志，`catalog` 保持 `None`。
        let catalog = match JmCatalog::open(&paths.catalog_db_path()) {
            Ok(cat) => {
                tracing::info!(
                    path = %cat.path().display(),
                    "jm 候选池已接入"
                );
                Some(cat)
            }
            Err(err) => {
                tracing::warn!(
                    err_title = "jm 候选池未接入，自动补全不可用",
                    path = %paths.catalog_db_path().display(),
                    message = %err
                );
                None
            }
        };

        Ok(Self {
            paths,
            config: Arc::new(RwLock::new(config)),
            jm_client: Arc::new(RwLock::new(None)),
            download_manager: Arc::new(RwLock::new(None)),
            store,
            catalog,
            catalog_sync: Arc::new(CatalogSyncState::default()),
            events: EventBus::new(),
        })
    }

    /// 第二阶段构造：创建 `PicaClient` 与 `DownloadManager` 并注入。
    pub fn init_runtime(&self) -> anyhow::Result<()> {
        let client = JmClient::new(self.clone());
        *self.jm_client.write() = Some(client);

        let manager = DownloadManager::new(self.clone());
        *self.download_manager.write() = Some(manager);

        // 候选池自动补全调度。`catalog` 为 `None`（库没挂进来）时轮次空转，
        // 不会报错——这是刻意的，下载器既有功能不能因此受影响。
        crate::catalog_sync::spawn_scheduler(self.clone());

        Ok(())
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn config(&self) -> &Arc<RwLock<Config>> {
        &self.config
    }

    /// 读配置快照。
    pub fn config_read(&self) -> Config {
        self.config.read().clone()
    }

    pub fn save_config(&self, config: &Config) -> anyhow::Result<()> {
        config.save(&self.paths.config_path())?;
        *self.config.write() = config.clone();
        Ok(())
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }

    /// 取任务持久化层。廉价可克隆（内部是 `Arc`）。
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// 取 jm 候选池。未接入时为 `None`。
    pub fn catalog(&self) -> Option<&JmCatalog> {
        self.catalog.as_ref()
    }

    /// 取候选池补全的共享状态（运行中标记、轮数、回写计数）。
    pub fn catalog_sync_state(&self) -> &CatalogSyncState {
        &self.catalog_sync
    }

    /// 取 `PicaClient`。初始化后必然存在。
    pub fn jm_client(&self) -> JmClient {
        self.jm_client
            .read()
            .clone()
            .expect("JmClient 尚未初始化")
    }

    /// 取 `DownloadManager`。初始化后必然存在。
    pub fn download_manager(&self) -> DownloadManager {
        self.download_manager
            .read()
            .clone()
            .expect("DownloadManager 尚未初始化")
    }

    /// 配置变更后重建 HTTP 客户端（代理/API 地址变了需要重建）。
    pub fn reload_jm_client(&self) {
        self.jm_client().reload_client();
    }

    /// 配置变更后应用新的下载并发度（D8 的修复）。
    ///
    /// 旧实现会 `shutdown()` 旧 manager 再 `DownloadManager::new()` 一个新的。
    /// 问题在于 `shutdown()` 只是把任务标记成 `Cancelled`，**已经 spawn 出去、
    /// 正在跑的下载任务依然活着**，并继续持有旧信号量的 permit。于是旧信号量
    /// 不会释放、新信号量又被创建，实际并发变成配置值的两倍。
    ///
    /// 改成在同一个 manager 上原地调整 permit：信号量对象不变，
    /// 在跑的任务无感，并发度立刻生效。
    ///
    /// 方法名刻意不用 `reload_*`：一旦叫「重载」，后人很容易顺手改回
    /// 「重建一个新 manager」，那正是上面这个 bug 的复现路径。
    pub fn apply_concurrency(&self) -> anyhow::Result<()> {
        let (chapter_concurrency, img_concurrency) = {
            let config = self.config.read();
            (config.chapter_concurrency, config.img_concurrency)
        };

        if let Some(manager) = self.download_manager.read().clone() {
            manager.update_concurrency(chapter_concurrency, img_concurrency);
        }

        Ok(())
    }
}

/// 兼容原代码里的 `download_dir` 等路径取用。
impl AppContext {
    pub fn download_dir(&self) -> PathBuf {
        self.config.read().download_dir.clone()
    }

    pub fn logs_dir(&self) -> anyhow::Result<PathBuf> {
        Ok(self.paths.logs_dir())
    }
}

/// 让 `&Path` 上的 join 更顺手（避免到处写 `.to_path_buf()`）。
pub fn join(base: &Path, child: impl AsRef<Path>) -> PathBuf {
    base.join(child.as_ref())
}