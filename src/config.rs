use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
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

/// Where settings come from: the process environment, falling back to an
/// optional `KEY=VALUE` file. The file is the Windows service's equivalent of
/// systemd's `EnvironmentFile` (services get no per-service environment).
#[derive(Debug, Clone, Default)]
pub struct Env {
    file: HashMap<String, String>,
}

impl Env {
    /// Just the process environment.
    pub fn process() -> Self {
        Self::default()
    }

    /// The process environment, then the variables in `path`.
    pub fn with_file(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading env file {}: {e}", path.display()))?;
        let file = parse_env_file(&text).map_err(|e| format!("env file {}: {e}", path.display()))?;
        Ok(Self { file })
    }

    /// A non-empty value for `name`, if set.
    pub fn get(&self, name: &str) -> Option<String> {
        env::var(name)
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| self.file.get(name).filter(|s| !s.is_empty()).cloned())
    }

    fn var_or(&self, name: &str, default: &str) -> String {
        self.get(name).unwrap_or_else(|| default.to_owned())
    }

    fn parse_or<T>(&self, name: &str, default: T) -> Result<T, String>
    where
        T: FromStr,
        T::Err: std::fmt::Display,
    {
        match self.get(name) {
            Some(s) => s.parse().map_err(|e| format!("{name} `{s}` is invalid: {e}")),
            None => Ok(default),
        }
    }
}

/// `KEY=VALUE` lines; blank lines and `#` comments are skipped, and a value
/// may be wrapped in single or double quotes.
pub fn parse_env_file(text: &str) -> Result<HashMap<String, String>, String> {
    let mut vars = HashMap::new();
    for (n, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| format!("line {}: expected KEY=VALUE", n + 1))?;
        let value = value.trim();
        let value = ['"', '\'']
            .iter()
            .find_map(|q| value.strip_prefix(*q).and_then(|v| v.strip_suffix(*q)))
            .unwrap_or(value);
        vars.insert(key.trim().to_owned(), value.to_owned());
    }
    Ok(vars)
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
    /// arduino-router's socket, through which the LED matrix firmware is
    /// reached. Unset (e.g. on Windows) = no LED matrix.
    pub matrix_router: Option<PathBuf>,
    pub app: AppSettings,
}

pub const MIN_ADMIN_TOKEN_LEN: usize = 32;

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::load(&Env::process())
    }

    pub fn load(env: &Env) -> Result<Self, String> {
        let admin_token = env.get("ADMIN_TOKEN").ok_or_else(|| {
            format!("ADMIN_TOKEN is not set (it must be at least {MIN_ADMIN_TOKEN_LEN} characters)")
        })?;
        if admin_token.len() < MIN_ADMIN_TOKEN_LEN {
            return Err(format!(
                "ADMIN_TOKEN is too short ({} chars, need at least {MIN_ADMIN_TOKEN_LEN})",
                admin_token.len()
            ));
        }

        let mut app = AppSettings::new(admin_token, env.var_or("PUBLIC_BASE_URL", "https://localhost:3443"));
        app.cors_origin = env.get("CORS_ORIGIN");
        if let Some(origin) = &app.cors_origin {
            axum::http::HeaderValue::from_str(origin)
                .map_err(|_| format!("CORS_ORIGIN `{origin}` is not a valid origin"))?;
        }
        app.image_workers = env.parse_or("IMAGE_WORKERS", 2usize)?.max(1);
        app.min_free_disk_bytes = env.parse_or("MIN_FREE_DISK_MB", 1024u64)? * 1024 * 1024;
        app.upload_burst = env.parse_or("UPLOAD_BURST", 5u32)?.max(1);
        app.upload_refill_secs = env.parse_or("UPLOAD_REFILL_SECS", 12u64)?.max(1);

        Ok(Self {
            bind_addr: env.parse_or("BIND_ADDR", SocketAddr::from(([0, 0, 0, 0], 3443)))?,
            tls_cert_path: env.var_or("TLS_CERT_PATH", "./certs/cert.pem").into(),
            tls_key_path: env.var_or("TLS_KEY_PATH", "./certs/key.pem").into(),
            http_redirect_addr: env
                .get("HTTP_REDIRECT_ADDR")
                .map(|s| s.parse().map_err(|e| format!("HTTP_REDIRECT_ADDR `{s}` is invalid: {e}")))
                .transpose()?,
            acme_webroot: env.get("ACME_WEBROOT").map(PathBuf::from),
            database_url: env.var_or("DATABASE_URL", "sqlite://data/notes.db?mode=rwc"),
            upload_dir: env.var_or("UPLOAD_DIR", "./uploads").into(),
            matrix_router: env.get("MATRIX_ROUTER").map(PathBuf::from),
            app,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_file_parsing() {
        let vars = parse_env_file(
            "\u{feff}# comment\n\nADMIN_TOKEN = abc=def \r\nUPLOAD_DIR=\"C:\\ProgramData\\notes server\\uploads\"\nX='y'\nEMPTY=\n",
        )
        .unwrap();
        assert_eq!(vars["ADMIN_TOKEN"], "abc=def");
        assert_eq!(vars["UPLOAD_DIR"], "C:\\ProgramData\\notes server\\uploads");
        assert_eq!(vars["X"], "y");
        assert_eq!(vars["EMPTY"], "");
        assert!(parse_env_file("no equals sign").unwrap_err().contains("line 1"));
    }

    #[test]
    fn config_from_env_file() {
        let dir = std::env::temp_dir().join(format!("notes-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("env");
        std::fs::write(
            &path,
            "ADMIN_TOKEN=file-token-0123456789abcdef0123456789\nBIND_ADDR=127.0.0.1:8443\nPUBLIC_BASE_URL=https://notes.example.org/\n",
        )
        .unwrap();
        let config = Config::load(&Env::with_file(&path).unwrap()).unwrap();
        assert_eq!(config.bind_addr, SocketAddr::from(([127, 0, 0, 1], 8443)));
        assert_eq!(config.app.public_base_url, "https://notes.example.org");
        assert!(config.matrix_router.is_none());
        std::fs::write(&path, "ADMIN_TOKEN=short\n").unwrap();
        assert!(Config::load(&Env::with_file(&path).unwrap()).unwrap_err().contains("too short"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
