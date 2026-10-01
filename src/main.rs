use std::future::Future;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::ExitCode;
use std::time::Duration;

use axum_server::Handle;
use notes_server::config::{Config, Env};
use notes_server::matrix::{self, Matrix, Status};
use notes_server::{AppState, db, tls};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
mod winservice;

const CERT_RELOAD_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(30);
const LOG_FILES_KEPT: usize = 14;

const USAGE: &str = "\
notes-server - serves the notes website and API over HTTPS

USAGE:
    notes-server [--env-file <path>]
        Run in the foreground. Settings come from the environment, then
        from the env file (KEY=VALUE lines). Ctrl+C stops it.
    notes-server --service --env-file <path>      (Windows)
        Run as the Windows service; the service manager starts it this way.
    notes-server self-signed-cert --cert <cert.pem> --key <key.pem> <name>...
        Write a self-signed certificate for the given host names / IPs.
";

/// Resolves when the server should shut down gracefully.
pub type Shutdown = Pin<Box<dyn Future<Output = ()> + Send>>;

enum Command {
    Serve { env_file: Option<PathBuf>, service: bool },
    SelfSignedCert { cert: PathBuf, key: PathBuf, names: Vec<String> },
    Help,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut first = args.next();
    if first.as_deref() == Some("self-signed-cert") {
        let (mut cert, mut key, mut names) = (None, None, Vec::new());
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--cert" => cert = Some(args.next().ok_or("--cert needs a path")?.into()),
                "--key" => key = Some(args.next().ok_or("--key needs a path")?.into()),
                _ if arg.starts_with('-') => return Err(format!("unknown option `{arg}`")),
                _ => names.push(arg),
            }
        }
        return match (cert, key) {
            (Some(cert), Some(key)) if !names.is_empty() => Ok(Command::SelfSignedCert { cert, key, names }),
            _ => Err("self-signed-cert needs --cert, --key and at least one name".into()),
        };
    }
    let (mut env_file, mut service) = (None, false);
    while let Some(arg) = first.take().or_else(|| args.next()) {
        match arg.as_str() {
            "--env-file" => env_file = Some(args.next().ok_or("--env-file needs a path")?.into()),
            "--service" => service = true,
            "-h" | "--help" => return Ok(Command::Help),
            _ => return Err(format!("unknown argument `{arg}`")),
        }
    }
    Ok(Command::Serve { env_file, service })
}

fn main() -> ExitCode {
    let command = match parse_args(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("notes-server: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match command {
        Command::Help => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Command::SelfSignedCert { cert, key, names } => match self_signed_cert(&cert, &key, names) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("notes-server: {e}");
                ExitCode::FAILURE
            }
        },
        #[cfg(windows)]
        Command::Serve { env_file, service: true } => winservice::run(env_file),
        #[cfg(not(windows))]
        Command::Serve { service: true, .. } => {
            eprintln!("notes-server: --service is only for Windows; use systemd elsewhere");
            ExitCode::from(2)
        }
        Command::Serve { env_file, service: false } => {
            if serve(env_file.as_deref(), Box::pin(std::future::pending())) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

/// Runs the server until Ctrl+C, SIGTERM or `shutdown`. Returns whether it
/// started and stopped cleanly; errors are logged.
pub fn serve(env_file: Option<&Path>, shutdown: Shutdown) -> bool {
    let env = match env_file.map(Env::with_file).transpose() {
        Ok(env) => env.unwrap_or_else(Env::process),
        Err(e) => {
            eprintln!("notes-server: {e}");
            return false;
        }
    };
    let _log_guard = init_logging(&env);
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!("starting the async runtime: {e}");
            return false;
        }
    };
    runtime.block_on(async_main(env, shutdown))
}

/// Logs to daily files in `LOG_DIR` if set (the Windows service has no
/// console), else to stdout. The guard flushes the file writer on drop.
fn init_logging(env: &Env) -> Option<WorkerGuard> {
    let filter = env
        .get("RUST_LOG")
        .and_then(|f| EnvFilter::try_new(f).ok())
        .unwrap_or_else(|| EnvFilter::new("notes_server=info,tower_http=info"));
    let file = env.get("LOG_DIR").and_then(|dir| {
        tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("notes-server")
            .filename_suffix("log")
            .max_log_files(LOG_FILES_KEPT)
            .build(&dir)
            .map_err(|e| eprintln!("notes-server: logging to {dir}: {e}; using stdout"))
            .ok()
    });
    match file {
        Some(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(writer).init();
            Some(guard)
        }
        None => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                // No colour escape codes in journald.
                .with_ansi(std::io::stdout().is_terminal())
                .init();
            None
        }
    }
}

async fn async_main(env: Env, shutdown: Shutdown) -> bool {
    let config = Config::load(&env);
    let matrix = open_matrix(config.as_ref().ok(), &env);
    matrix.status(Status::Starting, 0, 0);

    let result = match config {
        Ok(config) => run(config, matrix.clone(), shutdown).await,
        Err(e) => Err(e.into()),
    };
    let ok = match result {
        Ok(()) => {
            matrix.status(Status::Stopping, 0, 0);
            tracing::info!("stopped");
            true
        }
        Err(e) => {
            tracing::error!("{e}");
            matrix.status(Status::Error, 0, 0);
            matrix.text("START FAILED");
            false
        }
    };
    // Let the UART thread send the last lines before the process exits.
    drop(matrix);
    tokio::time::sleep(Duration::from_millis(300)).await;
    ok
}

/// The LED matrix link, from the parsed config or, if the config is broken,
/// straight from MATRIX_ROUTER so a startup failure can still be shown.
fn open_matrix(config: Option<&Config>, env: &Env) -> Matrix {
    let socket = match config {
        Some(c) => c.matrix_router.clone(),
        None => env.get("MATRIX_ROUTER").map(Into::into),
    };
    socket.map_or_else(Matrix::disabled, Matrix::open)
}

async fn run(config: Config, matrix: Matrix, shutdown: Shutdown) -> Result<(), Box<dyn std::error::Error>> {
    tls::install_crypto_provider();

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
    let app = notes_server::app(state.clone());

    let handle = Handle::new();
    let redirect_handle = Handle::new();
    {
        let (handle, redirect_handle) = (handle.clone(), redirect_handle.clone());
        tokio::spawn(async move {
            tokio::select! {
                () = shutdown_signal() => {}
                () = shutdown => {}
            }
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
    let reporter = tokio::spawn(matrix::report(state, matrix));
    let served = notes_server::serve_https(app, config.bind_addr, tls_config, handle).await;
    reporter.abort();
    served?;

    pool.close().await;
    Ok(())
}

async fn shutdown_signal() {
    // A Windows service has no console; if Ctrl+C can't be watched, never fire.
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
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

/// Writes a self-signed certificate and its private key (PEM). Used for the
/// local hop behind a Cloudflare Tunnel, or for LAN-only use.
fn self_signed_cert(cert: &Path, key: &Path, names: Vec<String>) -> Result<(), String> {
    let certified = rcgen::generate_simple_self_signed(names).map_err(|e| format!("generating certificate: {e}"))?;
    for (path, pem) in [(cert, certified.cert.pem()), (key, certified.signing_key.serialize_pem())] {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        std::fs::write(path, pem).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    println!("wrote {} and {}", cert.display(), key.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Command, parse_args};

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn arguments() {
        assert!(matches!(parse(&[]), Ok(Command::Serve { env_file: None, service: false })));
        assert!(matches!(
            parse(&["--service", "--env-file", "C:\\x\\env"]),
            Ok(Command::Serve { env_file: Some(p), service: true }) if p.to_str() == Some("C:\\x\\env")
        ));
        assert!(matches!(
            parse(&["self-signed-cert", "--cert", "c.pem", "--key", "k.pem", "localhost", "127.0.0.1"]),
            Ok(Command::SelfSignedCert { names, .. }) if names == ["localhost", "127.0.0.1"]
        ));
        assert!(parse(&["self-signed-cert", "--cert", "c.pem"]).is_err());
        assert!(parse(&["--env-file"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert!(matches!(parse(&["--help"]), Ok(Command::Help)));
    }
}
