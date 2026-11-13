//! Startup capability probe for the external programs the app shells out to,
//! plus the crash report that replaces a swallowed panic.
//!
//! `git` powers skill import and official-extension sync. It used to be
//! assumed present, so a machine without it answered with a generic error —
//! and on macOS calling a missing `git` raises the Xcode Command Line Tools
//! dialog instead. The probe names the tool, says which feature it disables,
//! and the UI shows it.
//!
//! `tar` is deliberately absent: the backup archives in-process with the `tar`
//! and `flate2` crates, so no external archiver is required anywhere.
//!
//! The panic hook matters because a release build has no visible stderr: on
//! Windows the process is a GUI subsystem binary, and on macOS it is normally
//! launched from Finder. A panic was therefore an invisible exit. Every panic
//! now writes one report under `<store root>/logs/` before the default hook
//! runs.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

mod capabilities;

/// Directory holding crash reports: `<store root>/logs`, beside the state the
/// app already owns on every platform.
pub fn log_dir() -> Option<PathBuf> {
    crate::paths::OrquestraPaths::resolve()
        .ok()
        .map(|paths| paths.root.join("logs"))
}

/// One external program the app invokes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolProbe {
    /// Program name as it must appear on `PATH`.
    pub id: String,
    pub present: bool,
    /// First line of `<tool> --version`, trimmed. `None` when missing.
    pub version: Option<String>,
    /// Feature ids that stop working without it.
    pub disables: Vec<String>,
    /// How to install it, when missing.
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolReport {
    pub tools: Vec<ToolProbe>,
    /// Ids of the missing tools, so a caller can show one line instead of a list.
    pub missing: Vec<String>,
    /// True inside a Flatpak sandbox, where host binaries are invisible.
    pub sandboxed: bool,
    /// Operator-facing note for this environment, when it changes the rules.
    pub note: Option<String>,
}

/// Probe every external program the app shells out to. Never fails: a
/// missing binary is a reported capability, not an error.
#[tauri::command]
pub fn probe_external_tools() -> ToolReport {
    report()
}

/// Log one line at boot naming the missing tools and what they disable.
///
/// The UI reports the same thing per tool; this exists so the diagnosis is
/// already in the log when an operator asks why a button failed.
pub fn warn_missing_tools() {
    let tools = report();
    if tools.missing.is_empty() {
        return;
    }
    let disabled: Vec<String> = tools
        .tools
        .iter()
        .filter(|tool| !tool.present)
        .flat_map(|tool| tool.disables.iter().cloned())
        .collect();
    crate::log_warn!(
        "missing external tools: {} (disabled features: {})",
        tools.missing.join(", "),
        disabled.join(", ")
    );
}

fn report() -> ToolReport {
    report_with(None)
}

/// The probe, with an optional `PATH` override for the child process.
///
/// Overriding the search path instead of the test process's environment keeps
/// the "missing tool" case testable without mutating global state that other
/// tests (and other threads) read at the same time. Resolution itself lives in
/// [`crate::exec_env`], which also knows the login shell's `PATH`.
fn report_with(search_path: Option<&std::ffi::OsStr>) -> ToolReport {
    capabilities::report(search_path)
}

/// Install the crash hook: write a report, then let the default hook speak.
///
/// The panic info type is never named: it was renamed to `PanicHookInfo` in
/// Rust 1.81 and this crate's MSRV is lower, while the closure argument is
/// inferred, so spelling it is the only thing that would break the floor.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-string panic payload".to_string());
        let location = info
            .location()
            .map(|place| format!("{}:{}:{}", place.file(), place.line(), place.column()))
            .unwrap_or_else(|| "unknown".to_string());

        if let Some(directory) = log_dir() {
            let written = write_report(
                &directory,
                &message,
                &location,
                &format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH),
                env!("CARGO_PKG_VERSION"),
                unix_seconds(SystemTime::now()),
                &std::backtrace::Backtrace::force_capture().to_string(),
            );
            if let Ok(path) = written {
                eprintln!("orquestra: panic report written to {}", path.display());
            }
        }
        previous(info);
    }));
}

/// Write one crash report. `stamp` is unix seconds; the returned path is the
/// report that was created.
fn write_report(
    directory: &Path,
    message: &str,
    location: &str,
    platform: &str,
    version: &str,
    stamp: u64,
    backtrace: &str,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(directory)?;
    // One file per crash, named by second: two panics in the same second on
    // two threads append instead of overwriting each other's diagnosis.
    let path = directory.join(format!("crash-{stamp}.log"));
    let header = format!(
        "orquestra-ai {version} panic\n\
         when: {when} (unix {stamp})\n\
         where: {location}\n\
         platform: {platform}\n\
         message: {message}\n\
         --- backtrace ---\n",
        when = utc_timestamp(stamp),
    );
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    use std::io::Write;
    file.write_all(header.as_bytes())?;
    file.write_all(backtrace.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(path)
}

fn unix_seconds(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// `YYYY-MM-DDTHH:MM:SSZ` for a unix timestamp, without pulling a date crate
/// into a binary that only needs this one line.
fn utc_timestamp(stamp: u64) -> String {
    let days = (stamp / 86_400) as i64;
    let seconds_of_day = stamp % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_of_day / 3600,
        (seconds_of_day % 3600) / 60,
        seconds_of_day % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's
/// `civil_from_days`), valid for every timestamp this process can produce.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_names_every_external_tool() {
        let report = report();
        let ids: Vec<&str> = report.tools.iter().map(|tool| tool.id.as_str()).collect();
        for expected in ["git", "python", "node"] {
            assert!(ids.contains(&expected), "{expected} is not probed: {ids:?}");
        }
        assert_eq!(
            report.missing.len(),
            report.tools.iter().filter(|tool| !tool.present).count()
        );
        for tool in &report.tools {
            // A capability can answer `--version` and still be unusable (a
            // Python without `venv`), so presence implies a banner, not the
            // other way around.
            assert!(tool.version.is_some() || !tool.present);
            assert!(!tool.disables.is_empty(), "{} disables nothing", tool.id);
        }
    }

    /// A machine without `git` must get a structured "feature disabled"
    /// report, not a panic and not an empty one: the UI needs the tool name to
    /// explain itself. The child's `PATH` points at a directory that does not
    /// exist, so the lookup fails exactly as it would on a machine where the
    /// tool was never installed.
    #[test]
    fn missing_tool_is_reported_as_a_disabled_capability() {
        let empty_path = std::env::temp_dir().join("orquestra-probe-no-bin");
        assert!(!empty_path.exists(), "the fixture path must not exist");

        let report = report_with(Some(empty_path.as_os_str()));

        assert!(report.missing.contains(&"git".to_string()));
        let git = report
            .tools
            .iter()
            .find(|tool| tool.id == "git")
            .expect("git is still probed");
        assert!(!git.present);
        assert!(git.version.is_none());
        assert_eq!(git.disables, ["skill_sync", "skill_import"]);
        assert!(git.hint.as_deref().is_some_and(|hint| hint.contains("git")));
    }

    #[test]
    fn unix_timestamps_render_as_utc() {
        assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00Z");
        // 2024-02-29T12:34:56Z, a leap day on purpose.
        assert_eq!(utc_timestamp(1_709_210_096), "2024-02-29T12:34:56Z");
        assert_eq!(utc_timestamp(1_767_225_599), "2025-12-31T23:59:59Z");
    }

    #[test]
    fn crash_reports_are_written_once_per_second() {
        let directory = crate::paths::fresh_scratch_dir("crash-report").expect("scratch dir");
        let first = write_report(
            &directory,
            "boom",
            "src/main.rs:1:1",
            "linux (x86_64)",
            "0.1.0",
            1_700_000_000,
            "stack",
        )
        .expect("write report");
        assert_eq!(first.file_name().unwrap(), "crash-1700000000.log");

        let second = write_report(
            &directory,
            "boom again",
            "src/main.rs:2:1",
            "linux (x86_64)",
            "0.1.0",
            1_700_000_000,
            "stack",
        )
        .expect("write report");
        assert_eq!(second, first);

        let body = std::fs::read_to_string(&first).expect("read report");
        assert!(body.contains("message: boom"), "{body}");
        assert!(body.contains("message: boom again"), "{body}");
        assert!(body.contains("where: src/main.rs:1:1"), "{body}");
        assert!(body.contains("2023-11-14T22:13:20Z"), "{body}");
        std::fs::remove_file(&first).expect("cleanup");
    }
}
