use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::Duration;

use axum_server::Handle;
use notes_server::config::Config;
use notes_server::{AppState, db, tls};
use tracing_subscriber::EnvFilter;

const CERT_RELOAD_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("notes_server=info,tower_http=info")),
        )
        // No colour escape codes in journald.
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    tls::install_crypto_provider();
    let config = Config::from_env()?;

    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;

    let tls_config = tls::load(&config.tls_cert_path, &config.tls_key_path).await?;
    tls::spawn_reloader(
        tls_config.clone(),
        config.tls_cert_path.clone(),
        config.tls_key_path.clone(),
        CERT_RELOAD_INTERVAL,
    );

    let public_base_url = config.app.public_base_url.clone();
    let state = AppState::new(pool.clone(), &config.upload_dir, config.app)?;
    let app = notes_server::app(state);

    let handle = Handle::new();
    let redirect_handle = Handle::new();
    {
        let (handle, redirect_handle) = (handle.clone(), redirect_handle.clone());
        tokio::spawn(async move {
            shutdown_signal().await;
            tracing::info!("shutting down, draining connections");
            handle.graceful_shutdown(Some(SHUTDOWN_GRACE));
            redirect_handle.graceful_shutdown(Some(SHUTDOWN_GRACE));
        });
    }

    if let Some(addr) = config.http_redirect_addr {
        tracing::info!(%addr, "redirecting plain HTTP to {public_base_url}");
        let acme_webroot = config.acme_webroot.clone();
        tokio::spawn(async move {
            let served =
                tls::serve_redirect(addr, &public_base_url, acme_webroot.as_deref(), redirect_handle);
            if let Err(e) = served.await {
                tracing::error!(%addr, error = %e, "HTTP redirect listener failed");
            }
        });
    }

    tracing::info!(
        addr = %config.bind_addr,
        uploads = %config.upload_dir.display(),
        "notes-server listening on https"
    );
    notes_server::serve_https(app, config.bind_addr, tls_config, handle).await?;

    pool.close().await;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    // systemd stops services with SIGTERM.
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
