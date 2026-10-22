//! Native system tray: the app's panel presence on Linux, macOS and Windows.
//!
//! One persistent icon with a context menu:
//!
//! * **Show / Hide OrquestraAI** — toggles the main window and focuses it.
//! * **Bridge Status** — read-only submenu listing every harness and whether
//!   the bridge reaches it, sampled from the same code the footer uses.
//! * **Quit** — graceful exit, the only way to leave the process running after
//!   the window is hidden.
//!
//! Platform note: on Linux the tray is served by the StatusNotifierItem /
//! AppIndicator protocol, which has no click callback — the desktop
//! environment shows the menu on click instead, so every action is reachable
//! from the menu. The click handler below is what macOS and Windows use.

use tauri::{
    menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

use crate::bridge;

use std::sync::atomic::{AtomicBool, Ordering};

/// Tray id, so the icon can be addressed without keeping a handle around.
pub const TRAY_ID: &str = "orquestra-tray";

/// Window-close behavior synced from the frontend's `minimizeToTray` setting.
/// Defaults to hidden-to-tray so the retention daemon survives a close;
/// the frontend pushes its stored preference on every startup and toggle.
pub struct AppConfigState {
    minimize_to_tray: AtomicBool,
}

impl Default for AppConfigState {
    fn default() -> Self {
        Self {
            minimize_to_tray: AtomicBool::new(true),
        }
    }
}

/// Remember whether closing the window should hide to the tray instead of
/// exiting. Called by the settings UI on startup and on every toggle.
#[tauri::command]
pub fn set_minimize_to_tray(state: tauri::State<'_, AppConfigState>, enabled: bool) {
    state.minimize_to_tray.store(enabled, Ordering::SeqCst);
}

/// Close-request routing: hide to the tray when the operator asked for it,
/// otherwise clean up and exit as before. The tray menu's Show/Hide entry
/// brings a hidden window back.
pub fn on_close_requested<R: Runtime>(window: &tauri::Window<R>, api: &tauri::CloseRequestApi) {
    let hide = window
        .app_handle()
        .try_state::<AppConfigState>()
        .map(|state| state.minimize_to_tray.load(Ordering::SeqCst))
        .unwrap_or(true);
    if hide {
        api.prevent_close();
        let _ = window.hide();
    } else {
        crate::single_instance::cleanup();
        window.app_handle().exit(0);
    }
}

/// Label of the window the tray shows and hides.
const MAIN_WINDOW: &str = "main";

/// Build the tray icon and its menu. Called once from the app setup hook.
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = build_menu(app)?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID).menu(&menu);

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder
        .tooltip("OrquestraAI — universal AI harness control plane")
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => toggle_main_window(app),
            "quit" => {
                crate::single_instance::cleanup();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Desktop-only path; on Linux the panel shows the menu instead.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// The tray menu, rebuilt from the harness links every time it is created.
pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / Hide OrquestraAI", true, None::<&str>)?;
    let bridge = bridge_menu(app)?;
    let about = MenuItem::with_id(
        app,
        "about",
        format!("OrquestraAI v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, Some("CmdOrCtrl+Q"))?;
    // Bound to locals: the menu borrows them, so temporaries here would be
    // dropped before the menu is built.
    let above_bridge = PredefinedMenuItem::separator(app)?;
    let above_footer = PredefinedMenuItem::separator(app)?;

    let items: Vec<&dyn IsMenuItem<R>> = vec![
        &toggle,
        &above_bridge,
        &bridge,
        &above_footer,
        &about,
        &quit,
    ];

    Menu::with_items(app, &items)
}

/// Read-only submenu: one line per harness, plus the headline count.
fn bridge_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    let statuses = bridge::get_bridge_status().unwrap_or_default();
    let linked = statuses.iter().filter(|status| status.bridged).count();

    let headline = MenuItem::with_id(
        app,
        "bridge_headline",
        format!(
            "{linked} of {} harnesses linked",
            statuses.len().max(linked)
        ),
        false,
        None::<&str>,
    )?;

    let mut items: Vec<Box<dyn IsMenuItem<R>>> = Vec::new();
    items.push(Box::new(headline));

    if statuses.is_empty() {
        items.push(Box::new(MenuItem::with_id(
            app,
            "bridge_empty",
            "No harness detected yet",
            false,
            None::<&str>,
        )?));
    } else {
        items.push(Box::new(PredefinedMenuItem::separator(app)?));

        for status in &statuses {
            items.push(Box::new(MenuItem::with_id(
                app,
                format!("bridge_{}", status.id),
                harness_line(&status.label, status.installed, status.bridged),
                false,
                None::<&str>,
            )?));
        }
    }

    let refs: Vec<&dyn IsMenuItem<R>> = items.iter().map(|item| item.as_ref()).collect();
    Submenu::with_items(app, "Bridge Status", true, &refs)
}

/// `Codex — linked`, `Codex — not linked`, `Codex — not installed`.
fn harness_line(label: &str, installed: bool, bridged: bool) -> String {
    let state = if !installed {
        "not installed"
    } else if bridged {
        "linked"
    } else {
        "not linked"
    };

    format!("{label} — {state}")
}

/// Show and focus the main window, or hide it when it is already visible.
pub fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_lines_read_as_a_status_report() {
        assert_eq!(harness_line("Codex", true, true), "Codex — linked");
        assert_eq!(harness_line("Codex", true, false), "Codex — not linked");
        assert_eq!(
            harness_line("Hermes Agent", false, false),
            "Hermes Agent — not installed"
        );
        // An uninstalled harness that reports bridged still reads as blocked.
        assert_eq!(harness_line("Grok", false, true), "Grok — not installed");
    }
}
