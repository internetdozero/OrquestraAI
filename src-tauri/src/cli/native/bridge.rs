use super::{parse_options, rendered, usage, CliError};

pub(super) fn run(args: &[String]) -> Result<String, CliError> {
    if args.first().map(String::as_str) != Some("status") {
        return Err(usage("usage: orquestra bridge status [--json]".to_string()));
    }
    let options = parse_options(&args[1..], &[], &["--json"])?;
    let statuses = crate::bridge::get_bridge_status().map_err(CliError::Operation)?;
    let human = statuses
        .iter()
        .map(|status| {
            let install = if status.installed {
                "installed"
            } else {
                "not installed"
            };
            let bridge = if status.bridged {
                "bridged"
            } else {
                "not bridged"
            };
            format!("{}: {install}, {bridge}", status.label)
        })
        .collect::<Vec<_>>()
        .join("\n");
    rendered(&statuses, options.has("--json"), human)
}
