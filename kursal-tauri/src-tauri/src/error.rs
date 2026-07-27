use crate::dialog_bridge::{DialogRequest, ask};
use crate::dirs::logs_dir;
use kursal_core::KursalError;
use serde::Serialize;
use serde_json::json;
use std::{
    backtrace::Backtrace,
    fs, panic,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;

#[derive(Serialize)]
pub struct CommandError {
    code: &'static str,
    message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<KursalError> for CommandError {
    fn from(e: KursalError) -> Self {
        let code = match e {
            KursalError::Storage(_)
            | KursalError::DbTransaction(_)
            | KursalError::DbTable(_)
            | KursalError::DbStorage(_)
            | KursalError::DbCommit(_)
            | KursalError::DbOpen(_)
            | KursalError::Encoding(_) => "storage",
            KursalError::Crypto(_) | KursalError::Signal(_) | KursalError::Hex(_) => "crypto",
            KursalError::Network(_) | KursalError::Address(_) => "network",
            KursalError::Identity(_) => "identity",
            KursalError::Io(_) => "io",
            KursalError::Misc(_) => "unknown",
            KursalError::KeyMismatch => "key mismatch",
        };
        Self::new(code, e.to_string())
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::new("unknown", message)
    }
}

pub type Result<T> = std::result::Result<T, CommandError>;

//

const SENTRY_DSN: &str = "https://f1839caacbd34e0f9969696a3c6f561f@crashes.kursal.chat/1";

const MAX_BACKTRACE_CHARS: usize = 16_000;

fn crash_file_path() -> Result<PathBuf> {
    Ok(logs_dir()?.join("crash_report.txt"))
}

fn panic_message(info: &panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn build_crash_report(info: &panic::PanicHookInfo<'_>) -> String {
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "unknown".to_string());

    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("unnamed").to_string();

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut backtrace = Backtrace::force_capture().to_string();
    if backtrace.len() > MAX_BACKTRACE_CHARS {
        backtrace.truncate(MAX_BACKTRACE_CHARS);
        backtrace.push_str("\n… (truncated)");
    }

    format!(
        "Kursal {version}\nPlatform: {os} {arch}\nTimestamp: {timestamp} (unix)\nThread: {thread_name}\nLocation: {location}\n\nPanic:\n{message}\n\nBacktrace:\n{backtrace}",
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        message = panic_message(info),
    )
}

pub fn install_panic_hook() -> Result<()> {
    let crash_path = crash_file_path()?;

    panic::set_hook(Box::new(move |info| {
        let report = build_crash_report(info);
        eprintln!("{report}");
        let _ = fs::write(&crash_path, report);
    }));

    Ok(())
}

pub async fn check_pending_crash(app: &AppHandle) {
    let Ok(crash_path) = crash_file_path() else {
        return;
    };

    if !crash_path.exists() {
        return;
    }

    let report = fs::read_to_string(&crash_path).unwrap_or_default();

    if report.trim().is_empty() {
        let _ = fs::remove_file(&crash_path);
        return;
    }

    let send = ask(
        app,
        DialogRequest::confirm("crash_report", json!({ "report": report }), "warning"),
    )
    .await;

    if send {
        let _guard = sentry::init((
            SENTRY_DSN,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                traces_sample_rate: 0.0,
                ..Default::default()
            },
        ));
        sentry::configure_scope(|scope| {
            scope.set_tag("os", std::env::consts::OS);
            scope.set_tag("arch", std::env::consts::ARCH);
        });
        sentry::capture_message(&report, sentry::Level::Error);
        if let Some(client) = sentry::Hub::current().client() {
            client.flush(Some(std::time::Duration::from_secs(2)));
        }
    }

    let _ = fs::remove_file(&crash_path);
}
