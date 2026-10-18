use super::{health::*, sweep::*};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::symlink;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from("/var/tmp/testing")
            .join(format!("orquestra-janitor-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).expect("create Janitor test workspace under /var/tmp/testing");
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn janitor_payloads_match_the_frozen_ipc_contract() {
    let ram = MemoryStats {
        total_mb: 4096,
        used_mb: 2048,
        free_mb: 2048,
        used_percent: 50.0,
    };
    let tmp = TmpStats {
        path: "/tmp".to_string(),
        is_tmpfs: true,
        total_mb: 1024,
        used_mb: 128,
        free_mb: 896,
        used_percent: 12.5,
    };
    let cpu = CpuStats {
        cores: 8,
        load_percent: 12.5,
    };
    let process = TopProcess {
        pid: 42,
        name: "test".to_string(),
        rss_mb: 5,
    };
    let health = SystemHealth {
        ram: ram.clone(),
        swap: ram.clone(),
        tmp: tmp.clone(),
        cpu: cpu.clone(),
        top_processes: vec![process.clone()],
        alert: false,
    };
    let sweep = SweepReport {
        reclaimed_mb: 1.5,
        removed_count: 2,
        status: "Sweep completed.".to_string(),
    };
    let fixture: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/ipc-contract.json"))
            .expect("IPC contract fixture is valid JSON");
    let shapes = [
        ("MemoryStats", serde_json::to_value(ram).unwrap()),
        ("CpuStats", serde_json::to_value(cpu).unwrap()),
        ("TmpStats", serde_json::to_value(tmp).unwrap()),
        ("TopProcess", serde_json::to_value(process).unwrap()),
        ("SystemHealth", serde_json::to_value(health).unwrap()),
        ("SweepReport", serde_json::to_value(sweep).unwrap()),
    ];

    for (name, value) in shapes {
        let actual = value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected = fixture["payloadShapes"][name]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| field.as_str().unwrap().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "Janitor payload `{name}` drifted");
    }
}

#[test]
fn alert_thresholds_are_strict_and_cover_ram_and_tmp() {
    assert!(!should_alert(85.0, 80.0));
    assert!(should_alert(85.1, 20.0));
    assert!(should_alert(20.0, 80.1));
}

#[test]
fn proc_parsers_and_memory_math_handle_linux_formats() {
    let memory = parse_meminfo(
        "MemTotal: 8192 kB\nMemAvailable: 2048 kB\nSwapTotal: 1024 kB\nSwapFree: 256 kB\n",
    );
    let ram = memory_stats(
        required_kb(&memory, "MemTotal").unwrap(),
        memory["MemAvailable"],
    );
    let swap = memory_stats(memory["SwapTotal"], memory["SwapFree"]);
    assert_eq!((ram.total_mb, ram.used_mb, ram.free_mb), (8, 6, 2));
    assert_eq!(ram.used_percent, 75.0);
    assert_eq!((swap.total_mb, swap.used_mb), (1, 0));

    let mounts = "root / ext4 rw 0 0\ntmpfs /tmp tmpfs rw 0 0\n";
    assert_eq!(mount_type_for(mounts, "/tmp").as_deref(), Some("tmpfs"));
    assert_eq!(mount_type_for(mounts, "/tmpish").as_deref(), Some("ext4"));
    assert_eq!(parse_load_one("1.50 0.5 0.2 1/22 123").unwrap(), 1.5);
    assert!(parse_load_one("invalid").is_err());
}

#[test]
fn snapshot_reads_live_proc_and_tmp_metrics() {
    let health = snapshot().expect("Linux proc and /tmp metrics should be readable");
    assert!(health.ram.total_mb > 0);
    assert!(health.ram.used_percent.is_finite());
    assert!(health.swap.used_percent.is_finite());
    assert_eq!(health.tmp.path, "/tmp");
    assert!(health.cpu.cores > 0);
    assert!(health.top_processes.len() <= 5);
}

#[test]
fn sweep_only_removes_dead_sockets_and_aged_direct_files() {
    let test = TestRoot::new();
    let orquestra = test.0.join(".orquestra");
    let scratch = test.0.join("scratch");
    fs::create_dir_all(&orquestra).unwrap();
    fs::create_dir_all(&scratch).unwrap();

    let live_path = orquestra.join("live.sock");
    let listener = UnixListener::bind(&live_path).unwrap();
    let dead_path = orquestra.join("dead.sock");
    drop(UnixListener::bind(&dead_path).unwrap());
    let non_socket = orquestra.join("not-a-socket.sock");
    fs::write(&non_socket, "keep").unwrap();
    let live_table = format!(
        "Num RefCount Protocol Flags Type St Inode Path\n00000000: 00000002 00000000 00010000 0001 01 77 {}\n",
        live_path.display()
    );

    let old_file = scratch.join("old.log");
    fs::write(&old_file, "stale scratch").unwrap();
    let old_time = SystemTime::now() - Duration::from_secs(7 * 60 * 60);
    let file = fs::File::open(&old_file).unwrap();
    file.set_times(fs::FileTimes::new().set_modified(old_time))
        .unwrap();
    let recent_file = scratch.join("recent.log");
    fs::write(&recent_file, "keep").unwrap();
    let nested = scratch.join("nested");
    fs::create_dir_all(&nested).unwrap();
    let nested_old = nested.join("nested-old.log");
    fs::write(&nested_old, "do not recurse").unwrap();
    fs::File::open(&nested_old)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(old_time))
        .unwrap();
    let link = scratch.join("old-link");
    symlink(&old_file, &link).unwrap();

    let report = sweep_at(&orquestra, &scratch, &live_table, SystemTime::now()).unwrap();
    assert_eq!(report.removed_count, 2);
    assert!(!dead_path.exists());
    assert!(live_path.exists());
    assert!(non_socket.exists());
    assert!(!old_file.exists());
    assert!(recent_file.exists());
    assert!(nested_old.exists());
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(report.status, "Sweep completed.");
    drop(listener);
}

#[test]
fn sweep_does_not_follow_symlinked_roots_or_remove_future_dated_files() {
    let test = TestRoot::new();
    let scratch = test.0.join("scratch");
    fs::create_dir_all(&scratch).unwrap();
    let future_file = scratch.join("future.log");
    fs::write(&future_file, "keep").unwrap();
    fs::File::open(&future_file)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(3600)))
        .unwrap();
    let linked_root = test.0.join("linked-scratch");
    symlink(&scratch, &linked_root).unwrap();

    let report = sweep_at(
        &test.0.join("missing-orquestra"),
        &linked_root,
        "",
        SystemTime::now(),
    )
    .unwrap();
    assert_eq!(report.removed_count, 0);
    assert_eq!(report.status, "Sweep completed; no stale files found.");
    assert!(future_file.exists());
}
