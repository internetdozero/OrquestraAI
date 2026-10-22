use super::super::exec_policy::POLICY;
use super::validate_argv;

/// Every allowlisted binary has exactly one policy entry: no binary
/// may be allowed without an entry in the table.
#[test]
fn every_allowlisted_binary_has_a_policy_entry() {
    let mut binaries: Vec<&str> = POLICY.iter().map(|entry| entry.binary).collect();
    binaries.sort_unstable();
    assert_eq!(
        binaries,
        [
            "aws",
            "az",
            "doctl",
            "echo",
            "gcloud",
            "gh",
            "kubectl",
            "printf",
            "stripe",
            "terraform"
        ]
    );
}

/// Task #322: one allowed and one denied example per policy entry,
/// including global-flag-prefix bypasses and `--flag=value` forms.
#[test]
fn policy_table_allows_and_denies_per_entry() {
    // (program, args-after-program, must_pass, error_must_contain)
    let cases: &[(&str, &[&str], bool, &str)] = &[
        // gh
        ("gh", &["pr", "list"], true, ""),
        ("gh", &["repo", "clone", "x"], true, ""),
        ("gh", &["issue", "list"], true, ""),
        ("gh", &["run", "list"], true, ""),
        ("gh", &["release", "list"], true, ""),
        ("gh", &["api", "repos/x"], false, "`api`"),
        ("gh", &["alias", "set", "--shell"], false, "`alias`"),
        ("gh", &["extension", "install", "x"], false, "`extension`"),
        ("gh", &["config", "get", "x"], false, "`config`"),
        ("gh", &["auth", "status"], false, "`auth`"),
        (
            "gh",
            &["pr", "list", "--hostname", "evil.test"],
            false,
            "`--hostname`",
        ),
        (
            "gh",
            &["pr", "list", "--hostname=evil.test"],
            false,
            "`--hostname`",
        ),
        // aws
        ("aws", &["s3", "ls"], true, ""),
        ("aws", &["dynamodb", "list-tables"], true, ""),
        ("aws", &["lambda", "list-functions"], true, ""),
        ("aws", &["sts", "get-caller-identity"], true, ""),
        ("aws", &["configure", "set", "x", "y"], false, "`configure`"),
        (
            "aws",
            &["--region", "us-east-1", "configure", "set", "x", "y"],
            false,
            "`configure`",
        ),
        (
            "aws",
            &["--debug", "configure", "set", "x", "y"],
            false,
            "`configure`",
        ),
        (
            "aws",
            &["sts", "get-session-token"],
            false,
            "get-session-token",
        ),
        ("aws", &["sts", "assume-role"], false, "assume-role"),
        (
            "aws",
            &["s3", "ls", "--endpoint-url", "https://evil.test"],
            false,
            "`--endpoint-url`",
        ),
        (
            "aws",
            &["s3", "ls", "--endpoint-url=https://evil.test"],
            false,
            "`--endpoint-url`",
        ),
        (
            "aws",
            &["s3", "ls", "--profile", "other"],
            false,
            "`--profile`",
        ),
        (
            "aws",
            &["s3", "ls", "--no-verify-ssl"],
            false,
            "`--no-verify-ssl`",
        ),
        // az
        ("az", &["group", "list"], true, ""),
        ("az", &["vm", "list"], true, ""),
        ("az", &["storage", "account", "list"], true, ""),
        ("az", &["webapp", "list"], true, ""),
        ("az", &["account", "list"], true, ""),
        (
            "az",
            &["extension", "add", "--name", "x"],
            false,
            "`extension`",
        ),
        ("az", &["rest", "--method", "get"], false, "`rest`"),
        ("az", &["login"], false, "`login`"),
        ("az", &["config", "set", "x"], false, "`config`"),
        (
            "az",
            &["account", "get-access-token"],
            false,
            "get-access-token",
        ),
        (
            "az",
            &["account", "--output", "json", "get-access-token"],
            false,
            "get-access-token",
        ),
        (
            "az",
            &["account", "-o", "json", "get-access-token"],
            false,
            "get-access-token",
        ),
        (
            "az",
            &["account", "--query", "name", "get-access-token"],
            false,
            "get-access-token",
        ),
        (
            "az",
            &["account", "--output", "json", "get-access-token"],
            false,
            "get-access-token",
        ),
        (
            "az",
            &["account", "-o", "json", "get-access-token"],
            false,
            "get-access-token",
        ),
        (
            "az",
            &["account", "--query", "name", "get-access-token"],
            false,
            "get-access-token",
        ),
        // terraform
        ("terraform", &["version"], true, ""),
        ("terraform", &["fmt", "-check"], true, ""),
        (
            "terraform",
            &["apply"],
            false,
            "operator-registered directory",
        ),
        (
            "terraform",
            &["plan"],
            false,
            "operator-registered directory",
        ),
        (
            "terraform",
            &["-chdir=/tmp/x", "version"],
            false,
            "`-chdir`",
        ),
        // legacy-open entries keep working, with their carve-outs
        ("kubectl", &["get", "pods"], true, ""),
        ("gcloud", &["compute", "instances", "list"], true, ""),
        ("doctl", &["compute", "droplet", "list"], true, ""),
        ("stripe", &["prices", "list"], true, ""),
        ("gcloud", &["auth", "print-access-token"], false, "`auth`"),
        // smoke-test helpers take no subcommand
        ("echo", &["hello-array"], true, ""),
        ("printf", &["%d", "42"], true, ""),
        // ambiguous: unknown pre-subcommand flag, or none at all
        ("gh", &["--bogus", "pr", "list"], false, "cannot determine"),
        ("gh", &[], false, "needs a subcommand"),
    ];
    for (program, rest, must_pass, needle) in cases {
        let mut full = vec![program.to_string()];
        full.extend(rest.iter().map(|part| part.to_string()));
        let result = validate_argv(&full);
        if *must_pass {
            assert!(result.is_ok(), "{program} {rest:?} must pass: {result:?}");
        } else {
            let error = result.expect_err(&format!("{program} {rest:?} must be refused"));
            assert!(error.contains(program), "error names the binary: {error}");
            assert!(
                error.contains(needle),
                "error names the cause ({needle}): {error}"
            );
        }
    }
}
