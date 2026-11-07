// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// The headless entry point shares the workspace log macros with the library,
// which re-exports them from `orquestra-core`.
pub use orquestra_core::log_error;

/// Default a Wayland session to the native backend, but never overrule the
/// operator: an explicit `GDK_BACKEND` (for example `x11` to run through
/// XWayland, or `broadway`) is a deliberate choice and is left alone.
#[cfg(target_os = "linux")]
fn prefer_native_wayland_backend() {
    if std::env::var_os("GDK_BACKEND").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return;
    }
    // Called from `main` before any thread exists, so nothing can observe the
    // environment mid-change. Rust 2024 makes `set_var` unsafe for exactly
    // that reason; this call site satisfies the same invariant.
    std::env::set_var("GDK_BACKEND", "wayland");
}

fn main() {
    #[cfg(target_os = "linux")]
    prefer_native_wayland_backend();

    // Headless operator entry points: the same bridge code the UI invokes,
    // usable from a terminal so fusion can be scripted or verified over SSH.
    let args: Vec<String> = std::env::args().collect();
    if let Some(result) = orquestra_ai_lib::dispatch_native_cli(&args[1..]) {
        match result {
            Ok(output) => println!("{output}"),
            Err(orquestra_ai_lib::CliError::Usage(error)) => {
                crate::log_error!("{error}");
                std::process::exit(2);
            }
            Err(orquestra_ai_lib::CliError::Operation(error)) => {
                crate::log_error!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    match args.get(1).map(String::as_str) {
        Some("--bridge-status") => {
            match orquestra_ai_lib::bridge_status_json() {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    crate::log_error!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("--bridge-deploy") => {
            match orquestra_ai_lib::deploy_bridge_json() {
                Ok(report) => {
                    println!("{report}");
                }
                Err(error) => {
                    crate::log_error!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("--mcp-status") => {
            match orquestra_ai_lib::mcp_status_json() {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    crate::log_error!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("--mcp-set") => {
            // --mcp-set <id> <global|harness> <on|off>
            let (Some(id), Some(scope), Some(state)) = (args.get(2), args.get(3), args.get(4))
            else {
                crate::log_error!("usage: orquestra --mcp-set <id> <global|harness> <on|off>");
                std::process::exit(2);
            };

            let enabled = match state.as_str() {
                "on" | "true" | "enable" => true,
                "off" | "false" | "disable" => false,
                other => {
                    crate::log_error!("expected on|off, got {other}");
                    std::process::exit(2);
                }
            };

            match orquestra_ai_lib::set_mcp_state_cli(id, scope, enabled) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    crate::log_error!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("--preset") => {
            // --preset <id> (e.g. --preset stealth)
            let Some(id) = args.get(2) else {
                crate::log_error!("usage: orquestra --preset <id>");
                std::process::exit(2);
            };

            match orquestra_ai_lib::apply_preset_json(id) {
                Ok(report) => println!("{report}"),
                Err(error) => {
                    crate::log_error!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("--mcp-deploy") => {
            match orquestra_ai_lib::deploy_mcp_json() {
                Ok(report) => {
                    println!("{report}");
                }
                Err(error) => {
                    crate::log_error!("{error}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Some("--help" | "-h") => {
            println!(
                "OrquestraAI — universal AI harness control plane\n\n\
                 Usage: orquestra [legacy flags] | idea <sub> | sessions <sub> |\n\
                                   bridge <sub> | vault <sub> | mcp <sub> | skills <sub>\n\n\
                 \x20 (no flag)        Launch the desktop control plane\n\
                 \x20 --bridge-status   Print harness link state as JSON\n\
                 \x20 --bridge-deploy   Back up and link every installed harness (JSON report)\n\
                 \x20 --mcp-status      Print the master MCP catalog as JSON\n\
                 \x20 --mcp-deploy      Compile active MCPs into every harness config (JSON report)\n\
                 \x20 --mcp-set         Toggle one server: --mcp-set <id> <global|harness> <on|off>\n\
                 \x20 --preset          Apply a mission preset: --preset <id> (JSON report)\n\n\
                 \x20 idea list         Lists OPEN ideas only; --status all includes the rest\n\
                 \x20 idea add          idea add --project <name> --title <text> [--json]\n\
                 \x20 idea              get|update|status|delete|search|projects|export-md|metrics\n\
                 \x20 idea note-*       note-post|note-list|note-get|note-read|note-clear\n\
                 \x20 sessions list     sessions list [--harness <id>] [--search <text>] [--json]\n\
                 \x20 sessions purge    Dry run by default; --yes requires a filter\n\
                 \x20 vault             Blind vault for scripts: vault <list|set-enabled|http|exec>\n\
                 \x20 mcp               mcp <list|status|add|remove|inspect> [--json]\n\
                 \x20 skills            skills <command> [options]\n\
                 \x20 bridge status     Show bridge health; add --json to any subcommand"
            );
            return;
        }
        _ => {}
    }

    // A mistyped subcommand used to fall through to the desktop launcher, which
    // on a running instance activates it and exits 0 with no output at all — a
    // typo in a script looked like success. Reject unknown words instead; flags
    // and existing paths keep the launcher behavior.
    if let Some(word) = args.get(1) {
        if !word.starts_with('-') && !word.contains('/') && !std::path::Path::new(word).exists() {
            crate::log_error!("unknown command '{word}' (see --help)");
            std::process::exit(2);
        }
    }

    if let Ok(true) = orquestra_ai_lib::single_instance::try_activate_existing() {
        return;
    }

    orquestra_ai_lib::run()
}
