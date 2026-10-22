//! Per-binary subcommand + flag policy for blind `vault_exec`.
//!
//! The bare-name allowlist keeps arbitrary programs out, but provider CLIs
//! are themselves multi-tools: `gh alias set --shell`, `aws configure set
//! credential_process`, `az extension add` or `terraform apply` would all run
//! with the vault secret in their environment. This table reduces each
//! binary to the subcommands agents need and refuses credential-minting
//! operations outright — output scrubbing only knows the stored secret, not
//! derived tokens (`aws sts get-session-token`, `az account get-access-token`,
//! `gcloud auth print-access-token`).
//!
//! Subcommand discovery skips each binary's known global flags (and their
//! values), so `aws --region us-east-1 configure …` or `aws --debug sts …`
//! cannot slip past an `argv[1]` check. Anything that is not an explicitly
//! understood global flag before the subcommand fails closed as ambiguous.
//!
//! `kubectl`, `gcloud` (except `auth`), `doctl` and `stripe` keep their
//! current open behavior behind explicit `legacy_open` entries: tightening
//! them needs per-subcommand review.
//! Follow-up Idea: per-binary subcommand allowlists for kubectl, gcloud,
//! doctl and stripe (credential-minting audit per CLI).

use super::exec::{MAX_ARGV_LEN, MAX_ARG_CHARS};
mod azure;
use azure::azure_account_operation;

/// A global flag that may precede the subcommand: name plus whether it
/// consumes the following token as its value (`--flag value` vs bare).
type GlobalFlag = (&'static str, bool);

/// Denied flag plus why it is refused (checked in `--flag value` and
/// `--flag=value` forms, anywhere in argv).
type DeniedFlag = (&'static str, &'static str);

type Check = fn(binary: &str, sub: &str, rest: &[String]) -> Result<(), String>;

pub(super) struct BinaryPolicy {
    pub binary: &'static str,
    pub globals: &'static [GlobalFlag],
    pub denied_flags: &'static [DeniedFlag],
    /// When true the binary takes no subcommand (`echo`, `printf`).
    pub no_subcommand: bool,
    pub check: Check,
    /// Set for binaries that keep their pre-#322 open behavior; the reason
    /// documents what review is still owed.
    #[allow(dead_code)]
    pub legacy_open: Option<&'static str>,
}

fn deny(binary: &str, what: &str, why: &str) -> String {
    format!("vault exec {binary}: {what} is refused — {why}")
}

// ---------------------------------------------------------------------------
// Per-binary subcommand checks
// ---------------------------------------------------------------------------

fn check_gh(binary: &str, sub: &str, _rest: &[String]) -> Result<(), String> {
    match sub {
        "pr" | "issue" | "repo" | "run" | "release" => Ok(()),
        "api" => Err(deny(
            binary,
            "subcommand `api`",
            "raw API passthrough can read or mutate outside the reviewed subcommands",
        )),
        "alias" => Err(deny(
            binary,
            "subcommand `alias`",
            "`alias set --shell` binds shell payloads that escape the allowlist",
        )),
        "extension" => Err(deny(
            binary,
            "subcommand `extension`",
            "extensions execute third-party code with the vault secret in the environment",
        )),
        "config" | "auth" => Err(deny(
            binary,
            &format!("subcommand `{sub}`"),
            "it reads or writes ambient credential config instead of the vault secret",
        )),
        _ => Err(deny(
            binary,
            &format!("subcommand `{sub}`"),
            "it is not on the allowlist for `gh` (`pr`, `issue`, `repo`, `run`, `release`)",
        )),
    }
}

fn check_aws(binary: &str, sub: &str, rest: &[String]) -> Result<(), String> {
    match sub {
        "s3" | "dynamodb" | "lambda" => Ok(()),
        "configure" => Err(deny(
            binary,
            "subcommand `configure`",
            "it writes ambient credentials, including `credential_process` command execution",
        )),
        "sts" => {
            let op = first_positional(rest);
            match op.as_deref() {
                Some("get-caller-identity") => Ok(()),
                Some(other) => Err(deny(
                    binary,
                    &format!("`sts {other}`"),
                    "it mints derived credentials the output scrubber cannot know",
                )),
                None => Err(deny(
                    binary,
                    "bare `sts`",
                    "an `sts` operation is required and only `get-caller-identity` is allowed",
                )),
            }
        }
        _ => Err(deny(
            binary,
            &format!("subcommand `{sub}`"),
            "it is not on the allowlist for `aws` (`s3`, `dynamodb`, `lambda`, `sts get-caller-identity`)",
        )),
    }
}

fn check_az(binary: &str, sub: &str, rest: &[String]) -> Result<(), String> {
    match sub {
        "group" | "vm" | "storage" | "webapp" => Ok(()),
        // `account` metadata reads are operational; the token-minter is not.
        "account" => match azure_account_operation(rest) {
            Ok(Some(operation)) if operation == "get-access-token" => Err(deny(
                binary,
                "`account get-access-token`",
                "it prints a derived token the output scrubber cannot know",
            )),
            Ok(_) => Ok(()),
            Err(()) => Err(deny(
                binary,
                "ambiguous `account` options",
                "unknown global options are refused so they cannot hide a credential-minting operation",
            )),
        },
        "extension" => Err(deny(
            binary,
            "subcommand `extension`",
            "extensions load third-party code with the vault secret in the environment",
        )),
        "rest" => Err(deny(
            binary,
            "subcommand `rest`",
            "raw REST passthrough can read or mutate outside the reviewed subcommands",
        )),
        "login" | "config" => Err(deny(
            binary,
            &format!("subcommand `{sub}`"),
            "it mutates ambient credentials instead of using the vault secret",
        )),
        _ => Err(deny(
            binary,
            &format!("subcommand `{sub}`"),
            "it is not on the allowlist for `az` (`group`, `vm`, `storage`, `webapp`, `account`)",
        )),
    }
}

fn check_terraform(binary: &str, sub: &str, _rest: &[String]) -> Result<(), String> {
    match sub {
        // Follow-up Idea: operator-registered working directories for
        // `terraform plan` / `apply` (scoped `-chdir` + state review).
        "version" | "fmt" => Ok(()),
        _ => Err(deny(
            binary,
            &format!("subcommand `{sub}`"),
            "only `version` and `fmt` are safe without an operator-registered directory; `plan`/`apply` need one (see follow-up Idea)",
        )),
    }
}

fn check_gcloud(binary: &str, sub: &str, _rest: &[String]) -> Result<(), String> {
    if sub == "auth" {
        return Err(deny(
            binary,
            "subcommand `auth`",
            "it prints or mints derived credentials the output scrubber cannot know",
        ));
    }
    Ok(())
}

fn allow_rest(_binary: &str, _sub: &str, _rest: &[String]) -> Result<(), String> {
    Ok(())
}

/// First positional (non-flag) token of `rest`, for two-level subcommands
/// (`aws sts <op>`, `az account <op>`). Global values are skipped by shape:
/// a value never starts with `-`.
fn first_positional(rest: &[String]) -> Option<String> {
    rest.iter().find(|token| !token.starts_with('-')).cloned()
}

// ---------------------------------------------------------------------------
// Table
// ---------------------------------------------------------------------------

pub(super) const POLICY: &[BinaryPolicy] = &[
    BinaryPolicy {
        binary: "gh",
        globals: &[("--repo", true), ("-R", true)],
        denied_flags: &[(
            "--hostname",
            "it would redirect the call to a host outside the preset allowlist",
        )],
        no_subcommand: false,
        check: check_gh,
        legacy_open: None,
    },
    BinaryPolicy {
        binary: "aws",
        globals: &[("--region", true), ("--output", true), ("--debug", false)],
        denied_flags: &[
            (
                "--endpoint-url",
                "it would redirect API calls to an arbitrary endpoint",
            ),
            (
                "--profile",
                "it would load ambient credentials from shared config instead of the vault secret",
            ),
            (
                "--no-verify-ssl",
                "it would disable TLS certificate verification",
            ),
        ],
        no_subcommand: false,
        check: check_aws,
        legacy_open: None,
    },
    BinaryPolicy {
        binary: "az",
        globals: &[("--output", true)],
        denied_flags: &[],
        no_subcommand: false,
        check: check_az,
        legacy_open: None,
    },
    BinaryPolicy {
        binary: "terraform",
        globals: &[],
        denied_flags: &[(
            "-chdir",
            "plan/apply need an operator-registered directory (see follow-up Idea)",
        )],
        no_subcommand: false,
        check: check_terraform,
        legacy_open: None,
    },
    BinaryPolicy {
        binary: "kubectl",
        globals: &[
            ("--namespace", true),
            ("-n", true),
            ("--context", true),
            ("--output", true),
            ("-o", true),
        ],
        denied_flags: &[],
        no_subcommand: false,
        check: allow_rest,
        legacy_open: Some("per-subcommand review still owed (see follow-up Idea)"),
    },
    BinaryPolicy {
        binary: "gcloud",
        globals: &[
            ("--project", true),
            ("--zone", true),
            ("--region", true),
            ("--format", true),
            ("--quiet", false),
        ],
        denied_flags: &[],
        no_subcommand: false,
        check: check_gcloud,
        legacy_open: Some("open except `auth`; per-subcommand review still owed"),
    },
    BinaryPolicy {
        binary: "doctl",
        globals: &[],
        denied_flags: &[],
        no_subcommand: false,
        check: allow_rest,
        legacy_open: Some("per-subcommand review still owed (see follow-up Idea)"),
    },
    BinaryPolicy {
        binary: "stripe",
        globals: &[],
        denied_flags: &[],
        no_subcommand: false,
        check: allow_rest,
        legacy_open: Some("per-subcommand review still owed (see follow-up Idea)"),
    },
    BinaryPolicy {
        binary: "echo",
        globals: &[],
        denied_flags: &[],
        no_subcommand: true,
        check: allow_rest,
        legacy_open: None,
    },
    BinaryPolicy {
        binary: "printf",
        globals: &[],
        denied_flags: &[],
        no_subcommand: true,
        check: allow_rest,
        legacy_open: None,
    },
];

/// Enforce the table on `argv` (`argv[0]` is the bare binary name).
/// Keeps the bare-name/path/NUL/size checks of the caller; adds subcommand
/// and flag policy. No binary is allowed without a table entry.
pub(super) fn check_policy(program: &str, argv: &[String]) -> Result<(), String> {
    let policy = POLICY
        .iter()
        .find(|entry| entry.binary == program)
        .ok_or_else(|| {
            format!("vault exec program {program:?} has no exec policy entry and is not allowed")
        })?;
    if argv.len() > MAX_ARGV_LEN {
        return Err("vault exec needs a non-empty command array".to_string());
    }
    let args = &argv[1..];
    for token in args {
        if token.len() > MAX_ARG_CHARS {
            return Err("vault exec arguments must be non-empty NUL-free text".to_string());
        }
        let name = token.split('=').next().unwrap_or("");
        if let Some((_, why)) = policy.denied_flags.iter().find(|(flag, _)| *flag == name) {
            return Err(deny(program, &format!("flag `{name}`"), why));
        }
    }
    if policy.no_subcommand {
        return Ok(());
    }
    // Subcommand = first non-flag token, skipping known globals + values.
    let mut index = 0;
    while index < args.len() {
        let token = &args[index];
        if !token.starts_with('-') || token == "-" {
            break;
        }
        let name = token.split('=').next().unwrap_or("");
        match policy.globals.iter().find(|(global, _)| *global == name) {
            Some((_, takes_value)) => {
                index += 1;
                if !token.contains('=') && *takes_value {
                    // `--flag value`: skip the value too; a missing value
                    // leaves no trustworthy subcommand behind.
                    index += 1;
                }
                if index > args.len() {
                    return Err(format!(
                        "vault exec {program}: cannot determine the subcommand after global flag `{name}`"
                    ));
                }
            }
            None => {
                return Err(format!(
                    "vault exec {program}: cannot determine the subcommand \
                     (unexpected flag `{token}` before it); pass flags after the subcommand"
                ));
            }
        }
    }
    let sub = args.get(index).ok_or_else(|| {
        format!("vault exec {program} needs a subcommand (none found in the command array)")
    })?;
    if sub.starts_with('-') {
        return Err(format!(
            "vault exec {program}: cannot determine the subcommand (only flags present)"
        ));
    }
    (policy.check)(program, sub, &args[index + 1..])
}
