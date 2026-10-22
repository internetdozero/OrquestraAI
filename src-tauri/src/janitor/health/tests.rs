#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Pure-layer tests: `/proc` parsing, Darwin page arithmetic and the `ps`
//! parser. None of these need the OS they describe, so they run on every CI
//! host — the macOS collector is otherwise only exercised by a Mac runner.

use super::macos::{darwin_memory_stats, parse_ps, DarwinMemoryCounts};
use super::*;

#[test]
fn memory_uses_available_memory_and_clamps_free_to_total() {
    let values = parse_meminfo("MemTotal: 2048 kB\nMemAvailable: 512 kB\nSwapTotal: 0 kB\n");
    let ram = memory_stats(
        required_kb(&values, "MemTotal").unwrap(),
        values["MemAvailable"],
    );
    assert_eq!((ram.total_mb, ram.used_mb, ram.free_mb), (2, 1, 0));
    assert_eq!(ram.used_percent, 75.0);
    let zero = memory_stats(0, 20);
    assert_eq!(zero.used_percent, 0.0);
}

#[test]
fn mounts_choose_the_longest_decoded_mountpoint() {
    let mounts = "root / ext4 rw 0 0\ntmpfs /tmp tmpfs rw 0 0\n";
    assert_eq!(mount_type_for(mounts, "/tmp").as_deref(), Some("tmpfs"));
    assert_eq!(
        mount_type_for(mounts, "/tmp/subdir").as_deref(),
        Some("tmpfs")
    );
    assert_eq!(mount_type_for(mounts, "/tmpish").as_deref(), Some("ext4"));
    assert_eq!(decode_mount_field("/a\\040b"), "/a b");
}

#[test]
fn load_parser_reads_one_minute_average_and_rejects_invalid_input() {
    assert_eq!(parse_load_one("1.25 0.50 0.10 2/44 123").unwrap(), 1.25);
    assert!(parse_load_one("bad").is_err());
}

/// Activity Monitor semantics: free is idle + speculative, used is active +
/// wired + compressed, and neither is derived from the other.
#[test]
fn darwin_pages_split_free_and_used_like_activity_monitor() {
    let counts = DarwinMemoryCounts {
        free: 100_000,
        speculative: 20_000,
        active: 300_000,
        wired: 100_000,
        compressor: 50_000,
    };
    let stats = darwin_memory_stats(16 * 1024 * 1024 * 1024, 4096, counts);

    assert_eq!(stats.total_mb, 16_384);
    // 120_000 pages * 4 KiB = 480_000 KiB -> 468 MiB.
    assert_eq!(stats.free_mb, 468);
    // 450_000 pages * 4 KiB = 1_800_000 KiB -> 1757 MiB.
    assert_eq!(stats.used_mb, 1_757);
    assert!((stats.used_percent - 10.728_7).abs() < 0.01);
}

/// A kernel change that reports absurd counters must clamp, not overflow the
/// UI with a >100% figure.
#[test]
fn darwin_pages_never_exceed_the_machine() {
    let counts = DarwinMemoryCounts {
        free: u64::MAX / 2,
        speculative: u64::MAX / 2,
        active: u64::MAX / 2,
        wired: u64::MAX / 2,
        compressor: u64::MAX / 2,
    };
    let stats = darwin_memory_stats(8 * 1024 * 1024 * 1024, 4096, counts);
    assert_eq!(stats.total_mb, 8_192);
    assert_eq!(stats.used_mb, 8_192);
    assert_eq!(stats.free_mb, 8_192);
    assert_eq!(stats.used_percent, 100.0);

    let empty = darwin_memory_stats(0, 0, DarwinMemoryCounts::default());
    assert_eq!(empty.used_percent, 0.0);
}

#[test]
fn ps_rows_become_ranked_top_processes() {
    let output = concat!(
        "  123   5120 /usr/libexec/small\n",
        "   42  20480 /Applications/Foo Bar.app/Contents/MacOS/Foo\n",
        "    7  20480 /bin/tie\n",
        "not-a-row\n",
        "  9   1024\n",
        " 10 4096\n"
    );
    let processes = parse_ps(output);

    assert_eq!(processes.len(), 3, "malformed rows are dropped, not fatal");
    assert_eq!(
        processes
            .iter()
            .map(|process| process.pid)
            .collect::<Vec<_>>(),
        vec![7, 42, 123],
        "RSS descending, pid ascending on ties"
    );
    assert_eq!(
        processes[1].name, "/Applications/Foo Bar.app/Contents/MacOS/Foo",
        "spaces inside comm survive"
    );
    assert_eq!(processes[2].rss_mb, 5);
}
