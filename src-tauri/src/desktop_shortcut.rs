//! Windows desktop registration: a Start Menu shortcut.
//!
//! Windows has no launcher files either. What the Start menu, the search field
//! and `Win+R` index is a `.lnk` under
//! `%APPDATA%\Microsoft\Windows\Start Menu\Programs`.
//!
//! Writing one needs the `IShellLink` COM interfaces. Rather than hand-rolling
//! vtables, the shortcut is created by `WScript.Shell` through PowerShell — a
//! first-party component present on every supported Windows version. The script
//! text is built here, with PowerShell single-quote escaping, and is passed as
//! one argv entry: no shell re-parses it, so a path containing quotes, spaces
//! or `&` cannot become a second command.

use std::path::Path;
use std::process::Command;

/// Shortcut name shown in the Start menu.
const SHORTCUT_NAME: &str = "OrquestraAI.lnk";

/// Path relative to `%APPDATA%`.
#[cfg(windows)]
const PROGRAMS_DIR: &str = "Microsoft/Windows/Start Menu/Programs";

/// PowerShell script that creates the shortcut at `link`.
///
/// Paths are single-quoted with `''` for an embedded quote, which is
/// PowerShell's own escaping rule — the same rule applies inside `-Command`.
pub(super) fn shortcut_script(link: &Path, exe: &Path) -> String {
    let quote = |path: &Path| format!("'{}'", path.display().to_string().replace('\'', "''"));
    let target = quote(exe);
    let working_dir = parent_of(exe)
        .map(|dir| format!("'{}'", dir.replace('\'', "''")))
        .unwrap_or_else(|| target.clone());
    format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut({link}); \
         $s.TargetPath = {target}; \
         $s.WorkingDirectory = {working_dir}; \
         $s.IconLocation = {target} + ',0'; \
         $s.Description = 'OrquestraAI'; \
         $s.Save()",
        link = quote(link),
    )
}

/// Directory part of a Windows path, accepting both separators.
///
/// `Path::parent` follows the *host* separator, so a test on another OS could
/// not exercise this; the shortcut targets Windows regardless of where the
/// script text is built.
fn parent_of(path: &Path) -> Option<String> {
    let text = path.display().to_string();
    let cut = text.rfind(['\\', '/'])?;
    Some(text[..cut].to_string())
}

/// Create the shortcut if it is missing. Returns whether anything was written.
///
/// A GUI process spawning PowerShell must not flash a console window, hence
/// `CREATE_NO_WINDOW`.
pub(super) fn install_shortcut(programs_dir: &Path, exe: &Path) -> bool {
    let link = programs_dir.join(SHORTCUT_NAME);
    if link.is_file() {
        return false;
    }
    if std::fs::create_dir_all(programs_dir).is_err() {
        return false;
    }

    let script = shortcut_script(&link, exe);
    let mut command = Command::new("powershell");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-Command",
        &script,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .status()
        .map(|status| status.success() && link.is_file())
        .unwrap_or(false)
}

/// Entry point from [`crate::desktop`]: write the shortcut for this install.
#[cfg(windows)]
pub(crate) fn ensure_start_menu_shortcut() {
    let Some(appdata) = std::env::var_os("APPDATA") else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    install_shortcut(&std::path::PathBuf::from(appdata).join(PROGRAMS_DIR), &exe);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn script_points_at_the_exe_and_escapes_quotes() {
        let script = shortcut_script(
            Path::new(r"C:\Users\dev\Start Menu\Programs\OrquestraAI.lnk"),
            Path::new(r"C:\Users\dev\.local\bin\orquestra-ai.exe"),
        );
        assert!(
            script.contains(r"CreateShortcut('C:\Users\dev\Start Menu\Programs\OrquestraAI.lnk')")
        );
        assert!(script.contains(r"$s.TargetPath = 'C:\Users\dev\.local\bin\orquestra-ai.exe'"));
        assert!(
            script.contains(r"$s.WorkingDirectory = 'C:\Users\dev\.local\bin'"),
            "the shortcut must start in the binary's directory: {script}"
        );
        assert!(script.contains("$s.Save()"));
    }

    /// A home directory with an apostrophe must not end the PowerShell string.
    #[test]
    fn an_apostrophe_in_the_path_is_escaped() {
        let script = shortcut_script(
            Path::new(r"C:\Users\o'brien\Programs\OrquestraAI.lnk"),
            Path::new(r"C:\Users\o'brien\orquestra-ai.exe"),
        );
        assert!(
            script.contains(r"'C:\Users\o''brien\orquestra-ai.exe'"),
            "{script}"
        );
        assert!(
            !script.contains("o'brien"),
            "unescaped quote would break the string"
        );
    }

    /// An existing shortcut must be left alone: re-creating it on every launch
    /// would spawn PowerShell each time.
    #[test]
    fn an_existing_shortcut_is_left_untouched() {
        let dir = std::env::temp_dir().join(format!(
            "orquestra-shortcut-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let link = dir.join(SHORTCUT_NAME);
        std::fs::write(&link, b"existing").unwrap();

        assert!(!install_shortcut(
            &dir,
            std::path::Path::new("orquestra-ai.exe")
        ));
        assert_eq!(std::fs::read(&link).unwrap(), b"existing");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
