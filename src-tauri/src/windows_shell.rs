//! Windows shell integration: open a URL, and give the process an identity.
//!
//! `rundll32 url.dll,FileProtocolHandler` is the classic way to open a browser
//! from a Windows program, and Microsoft documents `rundll32` as
//! "not intended for direct use": it is a loader, it takes no error return,
//! and a URL that reaches it is parsed by code that was never written to be a
//! public entry point. `ShellExecuteW` is the supported call, it returns an
//! error code, and it needs no console window.
//!
//! The AppUserModelID is what lets the Start Menu shortcut, the taskbar button
//! and the tray icon group as OrquestraAI instead of as a generic console app.
//! A packaged (MSIX) build gets one from its manifest; an unpacked build has to
//! declare it, which is what [`declare_app_identity`] does at startup.

/// Matches the macOS bundle identifier, so both platforms report the same app.
pub(crate) const APP_USER_MODEL_ID: &str = "ai.orquestra.desktop";

/// Wide, NUL-terminated copy of `value`, the shape every Win32 `W`-suffixed
/// call expects.
pub(crate) fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Open `url` in the operator's default browser.
///
/// The URL is validated by the caller (`desktop::open_url`), which refuses
/// anything that is not http/https: `ShellExecuteW` would happily run a `.exe`
/// or hand the string to a registered protocol handler.
#[cfg(windows)]
pub(crate) fn open_url(url: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let operation = wide("open");
    let target = wide(url);
    // SAFETY: both strings are NUL-terminated and outlive the call; the
    // remaining arguments are intentionally null (no parameters, no working
    // directory, no parent window).
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // `ShellExecuteW` returns a value cast to HINSTANCE: anything above 32 is a
    // success, and the low values are the documented error codes.
    if result as usize > 32 {
        Ok(())
    } else {
        Err(format!(
            "ShellExecuteW could not open the URL (code {})",
            result as usize
        ))
    }
}

/// Declare this process's AppUserModelID.
///
/// Failure is not fatal: without it Windows simply falls back to grouping by
/// executable path, which is the behavior before this call existed.
#[cfg(windows)]
pub(crate) fn declare_app_identity() {
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let id = wide(APP_USER_MODEL_ID);
    // SAFETY: the string is NUL-terminated and lives for the duration of the
    // call; the API copies what it needs.
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_strings_are_nul_terminated_utf16() {
        let encoded = wide("ai.orquestra.desktop");
        assert_eq!(encoded.last(), Some(&0));
        assert_eq!(encoded.len(), "ai.orquestra.desktop".len() + 1);
        // An emoji is a surrogate pair: counting chars would truncate it.
        let accented = wide("ação");
        assert_eq!(accented.last(), Some(&0));
        assert_eq!(&accented[..2], &[0x61, 0xE7]);
    }

    #[test]
    fn the_identity_matches_the_bundle_identifier() {
        assert_eq!(APP_USER_MODEL_ID, "ai.orquestra.desktop");
    }
}
