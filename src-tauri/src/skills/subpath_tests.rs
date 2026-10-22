use super::*;

fn scratch(label: &str) -> PathBuf {
    let root = PathBuf::from("/var/tmp/testing")
        .join(format!("orquestra-subpath-{label}-{}", std::process::id()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(&root).expect("scratch");
    root
}

fn skill_at(root: &Path, rel: &str) {
    let dir = root.join(rel);
    fs::create_dir_all(&dir).expect("skill dir");
    fs::write(dir.join(SKILL_FILE), "# Demo\n").expect("skill file");
}

#[test]
fn fragments_split_and_sanitize() {
    let (repo, sub) = split_git_url(
        "https://github.com/internetdozero/orquestra-extensions.git#skills/modular-start",
    );
    assert_eq!(
        repo,
        "https://github.com/internetdozero/orquestra-extensions.git"
    );
    assert_eq!(sub.as_deref(), Some("skills/modular-start"));

    let (plain, none) = split_git_url("https://github.com/o/r.git");
    assert_eq!(plain, "https://github.com/o/r.git");
    assert!(none.is_none());

    for hostile in [
        "",
        "#",
        "../escape",
        "skills/../escape",
        "a//b",
        "a\\b",
        ".",
        "skills/./x",
    ] {
        let (_, sub) = split_git_url(&format!("https://github.com/o/r.git#{hostile}"));
        assert!(sub.is_none(), "{hostile:?} must be refused");
    }
}

#[test]
fn anchor_wins_and_never_escapes() {
    let root = scratch("anchor");
    skill_at(&root, "skills/alpha");
    skill_at(&root, "other");
    let found = locate_skill_dir(&root, Some("skills/alpha")).expect("anchor");
    assert_eq!(found, root.join("skills/alpha"));
    assert!(locate_skill_dir(&root, Some("missing")).is_none());
    assert!(locate_skill_dir(&root, Some("../escape")).is_none());
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn a_symlinked_skill_directory_is_not_selected() {
    let root = scratch("link");
    skill_at(&root, "outside");
    let outside = root.join("outside");
    fs::write(outside.join(SKILL_FILE), "secret-outside\n").expect("marker");
    let staging = root.join("repo");
    fs::create_dir_all(&staging).expect("staging");
    std::os::unix::fs::symlink(&outside, staging.join("linked")).expect("dir symlink");
    std::os::unix::fs::symlink(&outside, staging.join("skills")).expect("container symlink");
    assert!(locate_skill_dir(&staging, Some("linked")).is_none());
    assert!(locate_skill_dir(&staging, None).is_none());

    let hooked = staging.join("hooked");
    fs::create_dir_all(&hooked).expect("hooked");
    std::os::unix::fs::symlink(outside.join(SKILL_FILE), hooked.join(SKILL_FILE))
        .expect("file symlink");
    assert!(locate_skill_dir(&staging, Some("hooked")).is_none());

    skill_at(&staging, "skills-real/kept");
    // `skills` is the symlink container; a real folder with another name still resolves.
    let found = locate_skill_dir(&staging, Some("skills-real/kept")).expect("real skill");
    assert_eq!(found, staging.join("skills-real/kept"));
    assert!(outside.join(SKILL_FILE).is_file(), "the target stays put");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn discovers_standalone_and_monorepo_layouts() {
    let root = scratch("layouts");
    assert!(locate_skill_dir(&root, None).is_none());

    skill_at(&root, "nested/deep/skill");
    let found = locate_skill_dir(&root, None).expect("deep skill");
    assert_eq!(found, root.join("nested/deep/skill"));

    let mono = scratch("mono");
    skill_at(&mono, "skills/beta");
    skill_at(&mono, "skills/alpha");
    let found = locate_skill_dir(&mono, None).expect("monorepo skill");
    assert_eq!(found, mono.join("skills/alpha"), "sorted first wins");
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&mono);
}

#[test]
fn root_still_wins_for_standalone_repos() {
    let root = scratch("root");
    skill_at(&root, ".");
    skill_at(&root, "skills/later");
    let found = locate_skill_dir(&root, None).expect("root skill");
    assert_eq!(found, root);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn refresh_does_not_pull_through_a_symlinked_git_dir() {
    let root = scratch("gitlink");
    let remote = root.join("remote.git");
    let seed = root.join("seed");
    let other = root.join("other");
    let skill = root.join("skill");
    let run = |dir: &Path, args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git");
        assert!(
            output.status.success(),
            "{args:?} {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&root, &["init", "--bare", remote.to_str().unwrap()]);
    run(
        &root,
        &["clone", remote.to_str().unwrap(), seed.to_str().unwrap()],
    );
    run(&seed, &["config", "user.email", "t@example.com"]);
    run(&seed, &["config", "user.name", "t"]);
    fs::write(seed.join("seed.txt"), "seed\n").unwrap();
    run(&seed, &["add", "seed.txt"]);
    run(&seed, &["commit", "-qm", "seed"]);
    run(&seed, &["push", "origin", "HEAD"]);
    run(
        &root,
        &["clone", remote.to_str().unwrap(), other.to_str().unwrap()],
    );
    fs::write(seed.join("seed.txt"), "more\n").unwrap();
    run(&seed, &["commit", "-qam", "more"]);
    run(&seed, &["push", "origin", "HEAD"]);
    fs::create_dir_all(&skill).unwrap();
    std::os::unix::fs::symlink(other.join(".git"), skill.join(".git")).unwrap();
    let head = |dir: &Path| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    let before = head(&other);
    let result = refresh_skill_dir(&skill, &root.join("missing-remote").to_string_lossy(), None);
    assert!(
        result.is_err(),
        "a linked .git must not count as a checkout"
    );
    assert_eq!(
        head(&other),
        before,
        "pull must not fast-forward the linked repository"
    );
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn refresh_does_not_pull_through_a_symlinked_vault_dir() {
    let root = scratch("vaultlink");
    let remote = root.join("remote.git");
    let seed = root.join("seed");
    let other = root.join("other");
    let vault = root.join("vault");
    let run = |dir: &Path, args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git");
        assert!(
            output.status.success(),
            "{args:?} {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&root, &["init", "--bare", remote.to_str().unwrap()]);
    run(
        &root,
        &["clone", remote.to_str().unwrap(), seed.to_str().unwrap()],
    );
    run(&seed, &["config", "user.email", "t@example.com"]);
    run(&seed, &["config", "user.name", "t"]);
    fs::write(seed.join("seed.txt"), "seed\n").unwrap();
    run(&seed, &["add", "seed.txt"]);
    run(&seed, &["commit", "-qm", "seed"]);
    run(&seed, &["push", "origin", "HEAD"]);
    run(
        &root,
        &["clone", remote.to_str().unwrap(), other.to_str().unwrap()],
    );
    fs::write(seed.join("seed.txt"), "more\n").unwrap();
    run(&seed, &["commit", "-qam", "more"]);
    run(&seed, &["push", "origin", "HEAD"]);
    fs::write(other.join("secret.txt"), "keep\n").unwrap();
    fs::create_dir_all(&vault).unwrap();
    let skill = vault.join("official");
    std::os::unix::fs::symlink(&other, &skill).unwrap();
    let head = |dir: &Path| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    let before = head(&other);
    let result = refresh_skill_dir(&skill, &root.join("missing-remote").to_string_lossy(), None);
    assert!(
        result.is_err(),
        "a symlinked vault folder must not be refreshed"
    );
    assert_eq!(
        head(&other),
        before,
        "pull must not fast-forward the linked repository"
    );
    assert_eq!(
        fs::read_to_string(other.join("secret.txt")).unwrap(),
        "keep\n"
    );
    let _ = fs::remove_dir_all(&root);
}
