//! Linux launcher/icon asset suite, kept beside [`crate::desktop_icons`] so
//! `desktop.rs` stays within the line cap.
//!
//! Every write here is symlink-hostile by design: an attacker-planted link
//! must never receive launcher bytes.

use crate::desktop_icons;

#[test]
fn desktop_install_does_not_write_through_a_symlink() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = orquestra_core::test_support::root().join(format!(
        "orquestra-desktop-link-{}-{unique}",
        std::process::id()
    ));
    let data = root.join("share");
    std::fs::create_dir_all(data.join("applications")).unwrap();
    let outside = root.join("outside.txt");
    let original = "ssh-private\n";
    std::fs::write(&outside, original).unwrap();
    let link = data.join("applications/orquestra.desktop");
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    desktop_icons::install_linux_assets(&data);

    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "the desktop entry symlink must stay"
    );
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), original);
    let regular = std::fs::read_to_string(data.join("applications/orquestra-ai.desktop")).unwrap();
    assert!(regular.contains("Name=OrquestraAI"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn desktop_install_does_not_write_into_a_symlinked_applications_dir() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = orquestra_core::test_support::root().join(format!(
        "orquestra-desktop-dir-link-{}-{unique}",
        std::process::id()
    ));
    let data = root.join("share");
    let outside = root.join("outside");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let link = data.join("applications");
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    desktop_icons::install_linux_assets(&data);

    assert!(
        !outside.join("orquestra.desktop").exists(),
        "install must not create a launcher in the symlink target"
    );
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    let _ = std::fs::remove_dir_all(&root);
}

/// Steady-state launches must not fire the icon-cache subprocesses: the
/// install reports a change only on the first run, so the refresh gate
/// in `refresh_integration_once` stays shut afterwards.
#[test]
fn desktop_install_reports_change_only_once() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let data = orquestra_core::test_support::root().join(format!(
        "orquestra-desktop-idem-{}-{unique}",
        std::process::id()
    ));

    assert!(
        desktop_icons::install_linux_assets(&data),
        "first install writes launcher and icons"
    );
    assert_eq!(
        std::fs::read_to_string(data.join("applications/orquestra.desktop")).unwrap(),
        desktop_icons::desktop_entry(&desktop_icons::launch_exec()),
    );
    assert!(
        !desktop_icons::install_linux_assets(&data),
        "nothing changed, so no cache refresh is warranted"
    );

    let _ = std::fs::remove_dir_all(&data);
}

/// Only the canonical launcher is menu-visible: the technical aliases
/// carry `NoDisplay=true`, so GNOME/KDE/XFCE show a single OrquestraAI
/// icon. A pre-dedup alias (canonical text from an older install)
/// converges to hidden on the next run.
#[test]
fn desktop_alias_entries_stay_hidden_from_menus() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let data = orquestra_core::test_support::root().join(format!(
        "orquestra-desktop-nodisplay-{}-{unique}",
        std::process::id()
    ));

    assert!(desktop_icons::install_linux_assets(&data));
    let canonical = std::fs::read_to_string(data.join("applications/orquestra.desktop")).unwrap();
    assert!(!canonical.contains("NoDisplay"), "canonical stays visible");
    for alias in ["orquestra-ai.desktop", "ai.orquestra.desktop.desktop"] {
        let text = std::fs::read_to_string(data.join(format!("applications/{alias}"))).unwrap();
        assert!(text.contains("Name=OrquestraAI"));
        assert!(
            text.lines().any(|line| line == "NoDisplay=true"),
            "{alias} must stay hidden from menus"
        );
    }

    // An older install left the canonical text in the alias: one pass
    // converges it to hidden, then the install is steady again.
    std::fs::write(
        data.join("applications/orquestra-ai.desktop"),
        desktop_icons::desktop_entry(&desktop_icons::launch_exec()),
    )
    .unwrap();
    assert!(
        desktop_icons::install_linux_assets(&data),
        "pre-dedup alias must converge to hidden"
    );
    let converged =
        std::fs::read_to_string(data.join("applications/orquestra-ai.desktop")).unwrap();
    assert!(converged.lines().any(|line| line == "NoDisplay=true"));
    assert!(
        !desktop_icons::install_linux_assets(&data),
        "steady state must not rewrite"
    );

    let _ = std::fs::remove_dir_all(&data);
}

/// The launcher must land in the directory the desktop actually reads:
/// `XDG_DATA_HOME` wins over `~/.local/share` whenever it is absolute.
#[test]
fn data_home_honours_xdg_and_ignores_relative_values() {
    let home = Some(std::path::PathBuf::from("/home/dev"));
    let data_home = |value: Option<&str>| {
        desktop_icons::data_home_with(
            |name| {
                assert_eq!(name, "XDG_DATA_HOME");
                value.map(std::ffi::OsString::from)
            },
            home.clone(),
        )
    };

    assert_eq!(
        data_home(Some("/nix/store/profile/share")),
        Some(std::path::PathBuf::from("/nix/store/profile/share"))
    );
    // The spec says a relative value is invalid, not that it is a path
    // relative to the home directory.
    assert_eq!(
        data_home(Some("relative/share")),
        Some(std::path::PathBuf::from("/home/dev/.local/share"))
    );
    assert_eq!(
        data_home(None),
        Some(std::path::PathBuf::from("/home/dev/.local/share"))
    );
    assert_eq!(
        desktop_icons::data_home_with(|_| None, None),
        None,
        "no home directory means no place to register"
    );
}
