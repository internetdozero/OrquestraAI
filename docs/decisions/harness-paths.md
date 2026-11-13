# Harness paths, one resolver

Every harness path used to be `home.join(".local/share/opencode")` or
`home.join(".config/manicode")`, spelled out at each call site. That is a
Linux answer to a three-platform question: macOS keeps both under
`~/Library/Application Support`, Windows splits them into `%LOCALAPPDATA%` and
`%APPDATA%`, and a Linux box with `XDG_DATA_HOME`/`XDG_CONFIG_HOME` set keeps
them wherever the operator said.

## The rule

`orquestra_core::paths` owns two functions and one mapping:

| Logical path       | Linux (`XDG_*_HOME` when absolute) | macOS                               | Windows                                          |
| ------------------ | ---------------------------------- | ----------------------------------- | ------------------------------------------------ |
| `.local/share/...` | `~/.local/share/...`               | `~/Library/Application Support/...` | `%LOCALAPPDATA%/...`, else `~/AppData/Local/...` |
| `.config/...`      | `~/.config/...`                    | `~/Library/Application Support/...` | `%APPDATA%/...`, else `~/AppData/Roaming/...`    |

- `data_dir()` / `config_dir()` answer where the **app** keeps its own data and
  configuration.
- `rebase(home, rel)` maps a home-relative harness path. The harness registry
  keeps its paths in XDG form, because that is how the harnesses document
  them, and every consumer calls `rebase` instead of `join`.

The two XDG prefixes are the _only_ thing rewritten. `.claude`, `.agents`,
`.copilot`, `.hermes`, `.grok`, `.codex` and `.gemini` are the
harnesses' own conventions; those folders sit beside the home directory on
every OS, and moving them would hide real stores.

A missing platform variable falls back to the profile directory rather than
failing: a stripped environment must still resolve inside the operator's home.
A relative `XDG_*` value is ignored, as the spec requires.

## Store overrides

`OPENCODE_DB` and `KILO_DB` name the database **file**. When set, they win over
the platform path and the artifact folders follow the file's directory. A
relative value resolves against the home the adapter was constructed with, so a
test fixture tree stays self-contained.

## Migration

Rebasing moves nothing on Linux, which is where every existing install lives.
On macOS and Windows the new location is checked first and the literal
`.config/...` location is kept as a fallback, so a store written by an earlier
build is still listed, still read, and still purged. The fallback can be
dropped once no build before this one is supported.

## Tests

`paths::dirs` carries a table test per OS branch (data root, config root, XDG
precedence, rebasing of each prefix). Adapter fixtures build their files
through the same `rebase`, so the suite exercises the platform path on every
runner instead of passing on Linux and testing nothing elsewhere.
