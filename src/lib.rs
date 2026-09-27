pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod imaging;
pub mod models;
pub mod routes;
pub mod storage;
pub mod tls;

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use sqlx::SqlitePool;
use tokio::sync::Semaphore;

use crate::config::AppSettings;
use crate::storage::LocalStorage;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub storage: LocalStorage,
    pub settings: Arc<AppSettings>,
    /// Bounds concurrent image processing across all requests (CPU + RAM).
    pub image_permits: Arc<Semaphore>,
}

impl AppState {
    /// Creates the upload directory if needed.
    pub fn new(db: SqlitePool, upload_dir: impl Into<PathBuf>, settings: AppSettings) -> io::Result<Self> {
        let upload_dir = upload_dir.into();
        std::fs::create_dir_all(&upload_dir)?;
        Ok(Self {
            db,
            storage: LocalStorage::new(upload_dir, &settings.public_base_url),
            image_permits: Arc::new(Semaphore::new(settings.image_workers.max(1))),
            settings: Arc::new(settings),
        })
    }
}

/// The full HTTP application. See [`routes::router`].
pub fn app(state: AppState) -> Router {
    routes::router(state)
}

/// Serves `app` over HTTPS until `handle` is shut down.
pub async fn serve_https(
    app: Router,
    addr: SocketAddr,
    tls: RustlsConfig,
    handle: Handle<SocketAddr>,
) -> io::Result<()> {
    axum_server::bind_rustls(addr, tls)
        .handle(handle)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
}
