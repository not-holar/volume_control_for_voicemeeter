//! Minimal logger: every line goes to stdout (visible when launched from a terminal)
//! and to a log file in the temp directory, so problems can be investigated even
//! when the app was started silently (Startup folder, Task Scheduler, ...).

use std::{fs::File, io::Write, sync::Mutex};

/// The log file is rotated when it grows past this size.
const MAX_LOG_SIZE: u64 = 1024 * 1024;

struct LogFile {
    file: File,
    written: u64,
}

static LOG_FILE: Mutex<Option<LogFile>> = Mutex::new(None);

pub fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join(concat!(env!("CARGO_PKG_NAME"), ".log"))
}

/// Keeps the previous log as `*.old.log` (useful after restarting the app because
/// something went wrong) and starts a new one.
fn open() -> Option<LogFile> {
    let path = log_path();
    let _ = std::fs::rename(&path, path.with_extension("old.log"));

    File::create(path)
        .ok()
        .map(|file| LogFile { file, written: 0 })
}

pub fn init() {
    *LOG_FILE.lock().unwrap_or_else(|e| e.into_inner()) = open();
}

pub fn write(args: std::fmt::Arguments) {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let line = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}  {args}\n",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    );

    print!("{line}");

    let mut guard = LOG_FILE.lock().unwrap_or_else(|e| e.into_inner());
    if guard.as_ref().is_some_and(|log| log.written > MAX_LOG_SIZE) {
        *guard = None; // close before renaming
        *guard = open();
    }
    if let Some(log) = guard.as_mut()
        && log.file.write_all(line.as_bytes()).is_ok()
    {
        log.written += line.len() as u64;
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::logging::write(format_args!($($arg)*))
    };
}
