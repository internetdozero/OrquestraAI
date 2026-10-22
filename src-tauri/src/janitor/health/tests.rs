#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

//! Pure-layer tests: `/proc` parsing, Darwin page arithmetic and the decoders
//! for libproc's buffers. None of these need the OS they describe, so they run
//! on every CI host — the macOS collector is otherwise only exercised by a Mac
//! runner.

use super::macos::{
    darwin_memory_stats, process_name, rank_by_rss, resident_bytes, DarwinMemoryCounts,
};
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

/// `proc_taskinfo` opens with `u64 pti_virtual_size` then `u64
/// pti_resident_size`; only the second word is the figure the panel shows.
#[test]
fn resident_size_comes_from_the_second_taskinfo_word() {
    let mut taskinfo = Vec::new();
    taskinfo.extend_from_slice(&999_u64.to_ne_bytes());
    taskinfo.extend_from_slice(&3_145_728_u64.to_ne_bytes());
    // Trailing struct fields the collector never reads.
    taskinfo.extend_from_slice(&[0xAB; 64]);

    assert_eq!(resident_bytes(&taskinfo), Some(3_145_728));
    assert_eq!(
        resident_bytes(&taskinfo[..16]),
        Some(3_145_728),
        "exactly the two leading words are enough"
    );
    assert_eq!(
        resident_bytes(&taskinfo[..15]),
        None,
        "a short read is refused, never truncated into a wrong number"
    );
}

#[test]
fn proc_name_stops_at_the_terminator_and_drops_empties() {
    assert_eq!(
        process_name(b"WindowServer\0stale").as_deref(),
        Some("WindowServer"),
        "bytes after the NUL are ignored"
    );
    assert_eq!(process_name(b"  spaced  \0").as_deref(), Some("spaced"));
    assert_eq!(process_name(b"unterminated"), Some("unterminated".into()));
    assert_eq!(process_name(b"\0tail"), None);
    assert_eq!(process_name(b"   \0"), None);
    assert_eq!(
        process_name(b"\xFF\xFE\0"),
        None,
        "invalid UTF-8 is skipped"
    );
}

#[test]
fn ranking_keeps_the_heaviest_and_breaks_ties_by_pid() {
    let mut processes = vec![
        TopProcess {
            pid: 123,
            name: "small".into(),
            rss_mb: 5,
        },
        TopProcess {
            pid: 42,
            name: "Foo".into(),
            rss_mb: 20,
        },
        TopProcess {
            pid: 7,
            name: "tie".into(),
            rss_mb: 20,
        },
    ];

    rank_by_rss(&mut processes, 2);
    assert_eq!(
        processes
            .iter()
            .map(|process| process.pid)
            .collect::<Vec<_>>(),
        vec![7, 42],
        "RSS descending, pid ascending on ties, tail truncated"
    );

    rank_by_rss(&mut processes, 10);
    assert_eq!(
        processes.len(),
        2,
        "a limit above the input keeps all of it"
    );
}
