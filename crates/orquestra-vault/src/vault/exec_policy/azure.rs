/// Azure CLI global options can appear between `account` and its operation.
/// Skip options with values so values such as `json` or `get-access-token`
/// are not misclassified as the operation itself.
pub(super) fn azure_account_operation(args: &[String]) -> Result<Option<String>, ()> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if matches!(
            arg.as_str(),
            "--output" | "-o" | "--query" | "--subscription" | "--tenant"
        ) {
            args.next();
            continue;
        }
        if matches!(
            arg.as_str(),
            "--help" | "-h" | "--verbose" | "--debug" | "--only-show-errors"
        ) || (arg.starts_with('-') && arg.contains('='))
        {
            continue;
        }
        if arg.starts_with('-') {
            return Err(());
        }
        return Ok(Some(arg.clone()));
    }
    Ok(None)
}
