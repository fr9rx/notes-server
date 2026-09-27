use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

/// Settings the HTTP application itself needs (as opposed to the listener).
#[derive(Debug, Clone)]
pub struct AppSettings {
    pub admin_token: String,
    /// Base used to build absolute image URLs, without a trailing slash.
    pub public_base_url: String,
    /// `None` = permissive CORS (development).
    pub cors_origin: Option<String>,
    pub image_workers: usize,
    pub min_free_disk_bytes: u64,
    /// Public uploads allowed in a burst per client IP.
    pub upload_burst: u32,
    /// Seconds to regain one upload slot.
    pub upload_refill_secs: u64,
}

impl AppSettings {
    /// Sensible defaults for tests and embedding; `admin_token` must still be supplied.
    pub fn new(admin_token: impl Into<String>, public_base_url: impl Into<String>) -> Self {
        Self {
            admin_token: admin_token.into(),
            public_base_url: public_base_url.into().trim_end_matches('/').to_owned(),
            cors_origin: None,
            image_workers: 2,
            min_free_disk_bytes: 1024 * 1024 * 1024,
            upload_burst: 5,
            upload_refill_secs: 12,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub tls_cert_path: PathBuf,
    pub tls_key_path: PathBuf,
    pub http_redirect_addr: Option<SocketAddr>,
    /// Served on the redirect listener for certbot `--webroot` renewals.
    pub acme_webroot: Option<PathBuf>,
    pub database_url: String,
    pub upload_dir: PathBuf,
    pub app: AppSettings,
}

pub const MIN_ADMIN_TOKEN_LEN: usize = 32;

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let admin_token = env::var("ADMIN_TOKEN").map_err(|_| {
            format!("ADMIN_TOKEN is not set (it must be at least {MIN_ADMIN_TOKEN_LEN} characters)")
        })?;
        if admin_token.len() < MIN_ADMIN_TOKEN_LEN {
            return Err(format!(
                "ADMIN_TOKEN is too short ({} chars, need at least {MIN_ADMIN_TOKEN_LEN})",
                admin_token.len()
            ));
        }

        let mut app = AppSettings::new(
            admin_token,
            var_or("PUBLIC_BASE_URL", "https://localhost:3443"),
        );
        app.cors_origin = env::var("CORS_ORIGIN").ok().filter(|s| !s.is_empty());
        if let Some(origin) = &app.cors_origin {
            axum::http::HeaderValue::from_str(origin)
                .map_err(|_| format!("CORS_ORIGIN `{origin}` is not a valid origin"))?;
        }
        app.image_workers = parse_or("IMAGE_WORKERS", 2usize)?.max(1);
        app.min_free_disk_bytes = parse_or("MIN_FREE_DISK_MB", 1024u64)? * 1024 * 1024;
        app.upload_burst = parse_or("UPLOAD_BURST", 5u32)?.max(1);
        app.upload_refill_secs = parse_or("UPLOAD_REFILL_SECS", 12u64)?.max(1);

        Ok(Self {
            bind_addr: parse_or("BIND_ADDR", SocketAddr::from(([0, 0, 0, 0], 3443)))?,
            tls_cert_path: var_or("TLS_CERT_PATH", "./certs/cert.pem").into(),
            tls_key_path: var_or("TLS_KEY_PATH", "./certs/key.pem").into(),
            http_redirect_addr: match env::var("HTTP_REDIRECT_ADDR") {
                Ok(s) if !s.is_empty() => Some(
                    s.parse()
                        .map_err(|e| format!("HTTP_REDIRECT_ADDR `{s}` is invalid: {e}"))?,
                ),
                _ => None,
            },
            acme_webroot: env::var("ACME_WEBROOT")
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
            database_url: var_or("DATABASE_URL", "sqlite://data/notes.db?mode=rwc"),
            upload_dir: var_or("UPLOAD_DIR", "./uploads").into(),
            app,
        })
    }
}

fn var_or(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn parse_or<T>(name: &str, default: T) -> Result<T, String>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(s) if !s.is_empty() => s
            .parse()
            .map_err(|e| format!("{name} `{s}` is invalid: {e}")),
        _ => Ok(default),
    }
}
