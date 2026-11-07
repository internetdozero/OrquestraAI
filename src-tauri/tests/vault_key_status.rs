//! The operator-facing answer to "is my vault still protected by the OS
//! keyring?". `with_temp_home` sets `ORQUESTRA_KEYRING_DISABLE=1`, the same
//! switch a headless host effectively hits, so this exercises the degraded
//! path end to end through the IPC command.

mod common;

use orquestra_ai_lib::vault::vault_key_status;

#[test]
fn a_host_without_a_keyring_reports_the_file_backend() {
    common::with_temp_home("vault-key-status", |home| {
        let status = vault_key_status().expect("status never fails on a missing keyring");

        assert_eq!(status.backend, "file-key");
        assert!(status.degraded, "a file key is the degraded backend");
        let reason = status.reason.expect("the degraded status explains itself");
        assert!(
            reason.to_lowercase().contains("keyring"),
            "the reason must name what is missing: {reason}"
        );

        // Reporting must not mint key material: the probe is read-only, so the
        // operator can look at the badge before ever storing a secret.
        assert!(
            !home.join(".orquestra/.file-key").exists(),
            "the status probe must not create the key file"
        );
    });
}
