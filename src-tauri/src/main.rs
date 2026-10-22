// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub(crate) mod log;

fn main() {
    // Headless operator entry points: the same bridge code the UI invokes,
    // usable from a terminal so fusion can be scripted or verified over SSH.
    let args: Vec<String> = std::env::args().collect();
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
                 Usage: orquestra [--bridge-status | --bridge-deploy | --mcp-status |\n                                   --mcp-deploy | --mcp-set <id> <scope> <on|off> |\n                                   --preset <id>]\n\n\
                 \x20 (no flag)        Launch the desktop control plane\n\
                 \x20 --bridge-status   Print harness link state as JSON\n\
                 \x20 --bridge-deploy   Back up and link every installed harness (JSON report)\n\
                 \x20 --mcp-status      Print the master MCP catalog as JSON\n\
                 \x20 --mcp-deploy      Compile active MCPs into every harness config (JSON report)\n\
                 \x20 --mcp-set         Toggle one server: --mcp-set <id> <global|harness> <on|off>\n\
                 \x20 --preset          Apply a mission preset: --preset <id> (JSON report)"
            );
            return;
        }
        _ => {}
    }

    if let Ok(true) = orquestra_ai_lib::single_instance::try_activate_existing() {
        return;
    }

    orquestra_ai_lib::run()
}
