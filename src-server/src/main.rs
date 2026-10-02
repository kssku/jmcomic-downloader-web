//! jmcomic-server 入口。
//!
//! 纯 API 后台：只提供 REST API + WebSocket，不再托管前端静态资源。
//!
//! 职责：
//! 1. 初始化路径 / 配置 / 日志 / 运行期组件（`AppContext`）。
//! 2. 组装 axum Router：REST API + WebSocket。
//! 3. 监听 HTTP 端口常驻。
//!
//! 环境变量：
//! - `JM_DATA_DIR`  数据根目录，默认 `./data`。Docker 里挂到 `/data`。
//! - `JM_PORT`      监听端口，默认 `8080`。
//! - `JM_BIND`      监听地址，默认 `0.0.0.0`。
//! - `JM_AUTH_TOKEN` 访问令牌，默认不校验（见 `JM_AUTH_DISABLED`）。
//! - `JM_AUTH_DISABLED` 设为 `false` 才开启令牌校验，默认 `true`（局域网免认证）。

use std::net::SocketAddr;

use anyhow::Context as _;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use jmcomic_server::api::routes;
use jmcomic_server::auth::AuthConfig;
use jmcomic_server::context::{AppContext, Paths};
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let paths = Paths::from_env().context("初始化路径失败")?;
    let app = AppContext::new(paths).context("加载配置失败")?;

    // 日志系统需要 AppContext 才能拿到日志目录，因此放在上下文之后。
    jmcomic_server::logger::init(&app).context("初始化日志失败")?;

    app.init_runtime().context("初始化运行期组件失败")?;

    // 返回的 bool 表示 Token 是否为随机生成（true = 没配环境变量）。
    let (auth, token_generated) = AuthConfig::from_env();
    let (bind, port) = bind_addr();

    let router = build_router(app.clone(), auth.clone());

    let addr: SocketAddr = format!("{bind}:{port}")
        .parse()
        .with_context(|| format!("解析监听地址 `{bind}:{port}` 失败"))?;

    // 随机生成的 Token 只在这里打印一次，之后不再出现在任何日志里。
    print_startup_banner(&addr, &auth, token_generated, &app);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("监听 `{addr}` 失败"))?;

    tracing::info!(%addr, "HTTP 服务已启动");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("HTTP 服务异常退出")?;

    tracing::info!("HTTP 服务已停止");
    Ok(())
}

/// 组装全部路由。
///
/// 层级顺序（从外到内）：
/// 1. `TraceLayer` —— 请求日志。
/// 2. `/api/*` 路由（内部自带认证中间件，默认关闭）。
/// 3. `/` —— 单文件控制台（编译期嵌入，无外部静态目录）。
fn build_router(app: AppContext, auth: AuthConfig) -> Router {
    // `routes::router` 已经带好 state 与认证中间件（含 `/ws`）。
    let api = Router::new().nest("/api", routes::router(app, auth));

    api.route("/", get(console))
        .layer(TraceLayer::new_for_http())
}

/// 单文件控制台。直接嵌进二进制，容器里不需要额外挂载目录。
async fn console() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

/// 监听地址，来自 `JM_BIND` / `JM_PORT`。
fn bind_addr() -> (String, u16) {
    let bind = std::env::var("JM_BIND").unwrap_or_else(|_| String::from("0.0.0.0"));
    let port = std::env::var("JM_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);
    (bind, port)
}

/// 启动横幅。随机 Token 只在这里出现一次。
fn print_startup_banner(addr: &SocketAddr, auth: &AuthConfig, token_generated: bool, app: &AppContext) {
    let data_dir = app.paths().data_dir.display().to_string();

    println!();
    println!("  ┌─────────────────────────────────────────────────────────┐");
    println!("  │  jmcomic-server  已启动（纯 API 后台）                    │");
    println!("  └─────────────────────────────────────────────────────────┘");
    println!("    API 地址   http://{addr}/api/");
    println!("    数据目录   {data_dir}");
    if auth.disabled {
        println!("    认证       已关闭（JM_AUTH_DISABLED 默认）");
    } else {
        println!("    用户名     {}", auth.username);
        if token_generated {
            println!("    访问令牌   {}  （随机生成，请立即保存）", auth.token);
        } else {
            println!("    访问令牌   来自环境变量 JM_AUTH_TOKEN");
        }
    }
    println!();
}

/// Ctrl-C 或 SIGTERM（Docker stop 发的是 SIGTERM）时优雅退出。
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(err) => {
                tracing::warn!(%err, "注册 SIGTERM 处理器失败");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("收到 Ctrl-C，开始优雅退出"),
        _ = terminate => tracing::info!("收到 SIGTERM，开始优雅退出"),
    }
}