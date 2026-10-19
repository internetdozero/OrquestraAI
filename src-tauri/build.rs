use std::path::Path;
use std::process::Command;

fn main() {
    let profile = std::env::var("PROFILE").unwrap_or_default();
    
    // Check whether this build was invoked via Tauri CLI or with custom-protocol
    let has_custom_protocol = std::env::var("CARGO_FEATURE_CUSTOM_PROTOCOL").is_ok();
    let is_tauri_cli = std::env::vars().any(|(k, _)| k.starts_with("TAURI_"));

    if profile == "release" {
        // Block raw cargo builds that lack both custom-protocol and Tauri CLI context
        if !has_custom_protocol && !is_tauri_cli {
            panic!(
                "\n\n\
                ========================================================================\n\
                ❌ [OrquestraAI Guardrail] Direct 'cargo build --release' is forbidden!\n\
                ========================================================================\n\
                Direct cargo build does not embed the frontend assets properly and\n\
                causes the release binary to fail at runtime with:\n\
                'Could not connect to 127.0.0.1: Connection refused'.\n\n\
                👉 To build the release executable, run:\n\
                   npm run build:release\n\
                ========================================================================\n\n"
            );
        }

        // Guardrail 2: Ensure frontend assets exist in ../dist/
        let dist_index = Path::new("../dist/index.html");
        if !dist_index.exists() {
            println!("cargo:warning=dist/index.html is missing! Triggering 'npm run build' automatically...");
            let status = Command::new("npm")
                .args(["run", "build"])
                .current_dir("..")
                .status();

            match status {
                Ok(s) if s.success() => {
                    println!("cargo:warning=Frontend built successfully via automatic guardrail.");
                }
                _ => {
                    panic!(
                        "\n\n❌ [OrquestraAI Guardrail]\n\
                        Frontend assets are required for release builds, but 'npm run build' failed!\n\
                        Build aborted to prevent generating a broken binary.\n\
                        Please fix frontend build errors or run 'npm run build' manually.\n"
                    );
                }
            }
        }
    }

    tauri_build::build();
}
