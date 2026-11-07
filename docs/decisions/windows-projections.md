# Decision: how skills project onto Windows

Status: accepted (W-01, task #19)
Scope: `src-tauri/src/skills/projection.rs`, `crates/orquestra-bridge`, `crates/orquestra-core/src/links.rs`

## Context

A projection is one directory entry that resolves to another: the vault is the
single copy of a skill, and each enabled scope (`global`, `codex`, `claude`, …)
holds a link to it. On Unix that link is a symlink, which any process can create
and which `lstat` reports without following.

Windows has two candidate primitives and neither is a drop-in replacement:

|                                    | Directory symlink (`mklink /D`)                                        | NTFS junction (`mklink /J`)                            |
| ---------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------ |
| Privilege required                 | `SeCreateSymbolicLinkPrivilege`, or Developer Mode (Windows 10 14972+) | none                                                   |
| Local volume only                  | no (also works across volumes)                                         | yes (both paths must be local)                         |
| Works for files                    | yes                                                                    | no, directories only                                   |
| Created by `std`                   | `std::os::windows::fs::symlink_dir`                                    | only `junction_point`, still nightly                   |
| Reported by `FileType::is_symlink` | yes                                                                    | **no**: a junction is a directory with a reparse point |

The last row is the one that matters for safety. Every `remove_dir_all` and
every "is this a real folder?" guard in the vault reads link state; if a
junction looks like a plain directory, disabling a skill or clearing a
projection would delete the vault copy it points at.

The third candidate — not linking at all, and injecting the vault path into each
harness config (`skills.paths`) — is what the current MCP/skills config merge
already does for OpenCode and Kilo. It works only for harnesses that read such a
config key; Codex, Claude, Copilot, Hermes and Grok discover `SKILL.md` files by
walking a folder, so config injection cannot reach them.

## Decision

Use **NTFS junctions plus a managed copy fallback**, behind one module.

1. `crates/orquestra-core/src/links.rs` is the only place that creates, detects,
   removes or reads a directory link. Unix → `std::os::unix::fs::symlink`;
   Windows → the `junction` crate (v1, unpinned privilege, no elevation).
2. Detection on Windows checks the `FILE_ATTRIBUTE_REPARSE_POINT` attribute
   rather than `FileType::is_symlink`. Junctions, directory symlinks and cloud
   placeholders then all count as links, which is the fail-closed answer for
   every guard that decides whether a folder may be deleted.
3. Removal on Windows uses `remove_dir` (which drops the reparse point and
   leaves the target untouched) before falling back to `remove_file`.
4. When a junction cannot be created (the vault and the harness live on
   different volumes, or the target is on a network share), fusion keeps the
   existing "refusing to replace" error instead of silently copying. A copy is
   planned, not implemented: it needs a sync step on every vault edit, and
   nothing in the acceptance criteria for this task depends on it yet.
5. All path comparisons go through `strip_verbatim`/`normalize`.
   `std::fs::canonicalize` returns `\\?\C:\…` on Windows, so an intact
   projection would otherwise look stale and be replaced on every status read.

## Consequences

- Projections, status and fusion keep one implementation, so a fix on one
  platform does not drift from the other.
- `is_dir_link` is intentionally broader than "is a symlink" on Windows: any
  reparse point is refused as a vault entry. Cloud-backed folders inside the
  vault therefore need a real local copy, which the vault already assumes.
- Junction targets must be local: a vault on a network drive cannot be
  projected. The refusal names the path and the operation instead of failing
  silently.
- `std::os::windows::fs::junction_point` will replace the dependency once it is
  stable; the module boundary is the only file that has to change.
