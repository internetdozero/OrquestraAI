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
//!
//! The shortcut also carries `System.AppUserModel.ID`. The process declares the
//! same id ([`crate::windows_shell::APP_USER_MODEL_ID`]); without it on both
//! sides the taskbar groups the app by executable path and the tray icon,
//! the window and the Start menu entry behave like three different programs.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::windows_shell::APP_USER_MODEL_ID;

/// Shortcut name shown in the Start menu.
const SHORTCUT_NAME: &str = "OwnHub.lnk";
/// Sidecar recording which script produced the shortcut.
const STAMP_NAME: &str = "OwnHub.lnk.stamp";

/// Path relative to `%APPDATA%`.
#[cfg(windows)]
const PROGRAMS_DIR: &str = "Microsoft/Windows/Start Menu/Programs";

/// PowerShell helper that stamps the shortcut's AppUserModelID.
///
/// `WScript.Shell` has no AUMID property: only the property store behind the
/// `.lnk` does, and reaching it means `SHGetPropertyStoreFromParsingName` plus
/// `IPropertyStore`. `Add-Type` compiles this once per launch in the PowerShell
/// that ships with Windows, so no toolchain is installed and no binary is
/// shipped. `PKEY_AppUserModel_ID` is the fixed GUID/PID pair below.
const APP_MODEL_HELPER: &str = r#"
Add-Type -Namespace OwnHub -Name AppModel -MemberDefinition @'
using System;
using System.Runtime.InteropServices;

[StructLayout(LayoutKind.Sequential, Pack = 4)]
public struct PropertyKey { public Guid fmtid; public int pid; }

[StructLayout(LayoutKind.Explicit)]
public struct PropVariant {
    [FieldOffset(0)] public short vt;
    [FieldOffset(8)] public IntPtr pointerValue;
}

[ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"),
 InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IPropertyStore {
    void GetCount(out uint count);
    void GetAt(uint index, out PropertyKey key);
    void GetValue(ref PropertyKey key, out PropVariant value);
    void SetValue(ref PropertyKey key, ref PropVariant value);
    void Commit();
}

public static class Store {
    [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = false)]
    static extern IPropertyStore SHGetPropertyStoreFromParsingName(
        string path, IntPtr reserved, int flags, ref Guid riid);

    [DllImport("ole32.dll")]
    static extern int PropVariantClear(ref PropVariant value);

    // GPS_READWRITE = 2. VT_LPWSTR = 31.
    public static void Set(string link, string id) {
        var iid = new Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99");
        var store = SHGetPropertyStoreFromParsingName(link, IntPtr.Zero, 2, ref iid);
        var key = new PropertyKey {
            fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"), pid = 5 };
        var value = new PropVariant {
            vt = 31, pointerValue = Marshal.StringToCoTaskMemUni(id) };
        store.SetValue(ref key, ref value);
        store.Commit();
        PropVariantClear(ref value);
    }
}
'@
"#;

/// PowerShell script that creates the shortcut at `link` with the app identity.
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
        "{helper}\
         $s = (New-Object -ComObject WScript.Shell).CreateShortcut({link}); \
         $s.TargetPath = {target}; \
         $s.WorkingDirectory = {working_dir}; \
         $s.IconLocation = {target} + ',0'; \
         $s.Description = 'OwnHub'; \
         $s.Save(); \
         [OwnHub.AppModel.Store]::Set({link}, '{aumid}')",
        helper = APP_MODEL_HELPER,
        link = quote(link),
        aumid = APP_USER_MODEL_ID,
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

/// 64-bit FNV-1a over the script text: identifies which build wrote a
/// shortcut, so a newer script refreshes an existing one exactly once.
fn fingerprint(script: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in script.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Does the shortcut need (re)writing?
///
/// A shortcut written by an earlier build carries the old target and no
/// AppUserModelID; treating "the file exists" as up to date would leave every
/// existing install without the identity forever.
fn needs_refresh(link: &Path, stamp: &Path, script: &str) -> bool {
    if !link.is_file() {
        return true;
    }
    match std::fs::read_to_string(stamp) {
        Ok(recorded) => recorded.trim() != fingerprint(script),
        Err(_) => true,
    }
}

/// Write the shortcut when it is missing or was produced by another script.
/// Returns whether anything was written.
///
/// A GUI process spawning PowerShell must not flash a console window, hence
/// `CREATE_NO_WINDOW`.
pub(super) fn install_shortcut(programs_dir: &Path, exe: &Path) -> bool {
    let link = programs_dir.join(SHORTCUT_NAME);
    let stamp = programs_dir.join(STAMP_NAME);
    let script = shortcut_script(&link, exe);
    if !needs_refresh(&link, &stamp, &script) {
        return false;
    }
    if std::fs::create_dir_all(programs_dir).is_err() {
        return false;
    }

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
    let written = command
        .status()
        .map(|status| status.success() && link.is_file())
        .unwrap_or(false);
    if written {
        let _ = std::fs::write(&stamp, fingerprint(&script));
    }
    written
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
    install_shortcut(&PathBuf::from(appdata).join(PROGRAMS_DIR), &exe);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn sandbox() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("ownhub-shortcut-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn script_points_at_the_exe_and_escapes_quotes() {
        let script = shortcut_script(
            Path::new(r"C:\Users\dev\Start Menu\Programs\OwnHub.lnk"),
            Path::new(r"C:\Users\dev\.local\bin\ownhub.exe"),
        );
        assert!(script.contains(r"CreateShortcut('C:\Users\dev\Start Menu\Programs\OwnHub.lnk')"));
        assert!(script.contains(r"$s.TargetPath = 'C:\Users\dev\.local\bin\ownhub.exe'"));
        assert!(
            script.contains(r"$s.WorkingDirectory = 'C:\Users\dev\.local\bin'"),
            "the shortcut must start in the binary's directory: {script}"
        );
        assert!(script.contains("$s.Save()"));
    }

    /// The window, the tray icon and the Start menu entry only group as one
    /// app when the shortcut carries the id the process declares.
    #[test]
    fn script_stamps_the_app_user_model_id_on_the_shortcut() {
        let script = shortcut_script(
            Path::new(r"C:\Users\dev\Programs\OwnHub.lnk"),
            Path::new(r"C:\Users\dev\ownhub.exe"),
        );
        assert!(
            script.contains(r#"[OwnHub.AppModel.Store]::Set('C:\Users\dev\Programs\OwnHub.lnk', 'dev.ownanai.hub')"#),
            "{script}"
        );
        // PKEY_AppUserModel_ID and the IPropertyStore IID: a wrong pair writes
        // to an unrelated property or fails to open the store at all.
        assert!(script.contains("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"));
        assert!(script.contains("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"));
        assert!(script.contains(&format!("'{}'", APP_USER_MODEL_ID)));
    }

    /// A home directory with an apostrophe must not end the PowerShell string.
    #[test]
    fn an_apostrophe_in_the_path_is_escaped() {
        let script = shortcut_script(
            Path::new(r"C:\Users\o'brien\Programs\OwnHub.lnk"),
            Path::new(r"C:\Users\o'brien\ownhub.exe"),
        );
        assert!(
            script.contains(r"'C:\Users\o''brien\ownhub.exe'"),
            "{script}"
        );
        assert!(
            !script.contains("o'brien"),
            "unescaped quote would break the string"
        );
    }

    /// An up-to-date shortcut must be left alone: re-creating it on every
    /// launch would spawn PowerShell each time.
    #[test]
    fn an_up_to_date_shortcut_is_left_untouched() {
        let dir = sandbox();
        let link = dir.join(SHORTCUT_NAME);
        std::fs::write(&link, b"existing").unwrap();
        let script = shortcut_script(&link, Path::new("ownhub.exe"));
        std::fs::write(dir.join(STAMP_NAME), fingerprint(&script)).unwrap();

        assert!(!install_shortcut(&dir, Path::new("ownhub.exe")));
        assert_eq!(std::fs::read(&link).unwrap(), b"existing");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A shortcut from an older build (no stamp, or a stale one) is rewritten:
    /// otherwise the AppUserModelID fix never reaches an existing install.
    #[test]
    fn a_shortcut_from_an_older_build_is_refreshed() {
        let dir = sandbox();
        let link = dir.join(SHORTCUT_NAME);
        let stamp = dir.join(STAMP_NAME);
        let script = shortcut_script(&link, Path::new("ownhub.exe"));

        assert!(needs_refresh(&link, &stamp, &script), "no shortcut yet");
        std::fs::write(&link, b"old").unwrap();
        assert!(needs_refresh(&link, &stamp, &script), "no stamp yet");
        std::fs::write(&stamp, "0000000000000000").unwrap();
        assert!(needs_refresh(&link, &stamp, &script), "stale stamp");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
