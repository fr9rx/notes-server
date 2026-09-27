use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use tower_http::services::ServeDir;

/// Installs `ring` as the process-wide rustls crypto provider. `ring` is
/// used instead of `aws-lc-rs` because it cross-compiles to the Pi without
/// cmake. Safe to call more than once.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub async fn load(cert: &Path, key: &Path) -> Result<RustlsConfig, String> {
    RustlsConfig::from_pem_file(cert, key).await.map_err(|e| {
        format!(
            "failed to load TLS certificate `{}` / key `{}`: {e} \
             (set TLS_CERT_PATH / TLS_KEY_PATH; see README \"Certificates\")",
            cert.display(),
            key.display()
        )
    })
}

/// Re-reads the certificate periodically so renewals (e.g. certbot) take
/// effect without a restart. A failed reload keeps the current certificate.
pub fn spawn_reloader(config: RustlsConfig, cert: PathBuf, key: PathBuf, every: Duration) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(every);
        tick.tick().await; // the first tick fires immediately; we just loaded it
        loop {
            tick.tick().await;
            match config.reload_from_pem_file(&cert, &key).await {
                Ok(()) => tracing::info!("reloaded TLS certificate"),
                Err(e) => tracing::warn!(error = %e, "TLS certificate reload failed; keeping the current one"),
            }
        }
    });
}

/// Plain-HTTP app that permanently redirects every request to the HTTPS
/// origin. The target comes from configuration, never the Host header.
///
/// With `acme_webroot`, `/.well-known/acme-challenge/*` is served from
/// `<acme_webroot>/.well-known/acme-challenge/` instead, so
/// `certbot --webroot -w <acme_webroot>` can renew certificates while this
/// listener holds port 80.
pub fn redirect_app(public_base_url: &str, acme_webroot: Option<&Path>) -> Router {
    let base = public_base_url.trim_end_matches('/').to_owned();
    let mut app = Router::new().fallback(move |uri: Uri| {
        let base = base.clone();
        async move {
            let path = uri.path_and_query().map_or("/", |pq| pq.as_str());
            (StatusCode::PERMANENT_REDIRECT, [(header::LOCATION, format!("{base}{path}"))])
        }
    });
    if let Some(root) = acme_webroot {
        app = app.nest_service(
            "/.well-known/acme-challenge",
            ServeDir::new(root.join(".well-known").join("acme-challenge")),
        );
    }
    app
}

pub async fn serve_redirect(
    addr: SocketAddr,
    public_base_url: &str,
    acme_webroot: Option<&Path>,
    handle: Handle<SocketAddr>,
) -> io::Result<()> {
    axum_server::bind(addr)
        .handle(handle)
        .serve(redirect_app(public_base_url, acme_webroot).into_make_service())
        .await
}
