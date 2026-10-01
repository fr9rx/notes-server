//! Runs notes-server as a Windows service. The service manager starts the
//! binary with `--service`; we hand control to its dispatcher, report
//! "running", and turn Stop/Shutdown requests into a graceful shutdown.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle};
use windows_service::{define_windows_service, service_dispatcher};

/// Must match the name the service is installed under (deploy/install-windows.ps1).
pub const SERVICE_NAME: &str = "notes-server";

/// How long a stop may take before the service manager gives up on us.
const STOP_WAIT_HINT: Duration = Duration::from_secs(35);

/// The service entry point gets no arguments of ours, so the env file path is
/// parked here by `run` before the dispatcher calls back.
static ENV_FILE: OnceLock<Option<PathBuf>> = OnceLock::new();

define_windows_service!(ffi_service_main, service_main);

/// Blocks until the service stops. Fails if not started by the service manager.
pub fn run(env_file: Option<PathBuf>) -> ExitCode {
    let _ = ENV_FILE.set(env_file);
    match service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "notes-server: --service only works when started by the Windows service manager ({e}).\n\
                 Use `sc.exe start {SERVICE_NAME}`, or run without --service in the foreground."
            );
            ExitCode::FAILURE
        }
    }
}

fn service_main(_args: Vec<OsString>) {
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let stop_tx = Mutex::new(Some(stop_tx));
    let handler = move |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            if let Some(tx) = stop_tx.lock().ok().and_then(|mut tx| tx.take()) {
                let _ = tx.send(());
            }
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let Ok(status) = service_control_handler::register(SERVICE_NAME, handler) else {
        return;
    };

    report(status, ServiceState::Running, ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN, 0);
    let shutdown = Box::pin(async move {
        let _ = stop_rx.await;
        report(status, ServiceState::StopPending, ServiceControlAccept::empty(), 0);
    });
    let env_file = ENV_FILE.get().cloned().flatten();
    let ok = crate::serve(env_file.as_deref(), shutdown);
    // A non-zero exit code makes the service manager apply the recovery
    // actions (restart), set by the installer.
    report(status, ServiceState::Stopped, ServiceControlAccept::empty(), if ok { 0 } else { 1 });
}

fn report(status: ServiceStatusHandle, state: ServiceState, accept: ServiceControlAccept, exit_code: u32) {
    let _ = status.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: accept,
        exit_code: ServiceExitCode::Win32(exit_code),
        checkpoint: 0,
        wait_hint: if state == ServiceState::StopPending { STOP_WAIT_HINT } else { Duration::ZERO },
        process_id: None,
    });
}
