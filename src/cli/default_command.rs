use std::ffi::OsString;

const COMMANDS: &[&str] = &[
    "audit",
    "check",
    "ci-template",
    "config",
    "config-schema",
    "coverage",
    "cycles",
    "dead-code",
    "decision-surface",
    "dupes",
    "explain",
    "fix",
    "flags",
    "health",
    "hooks",
    "impact",
    "init",
    "inspect",
    "list",
    "report-schema",
    "review",
    "rule-pack-schema",
    "schema",
    "security",
    "trace",
    "trace-clone",
    "trace-dependency",
    "trace-file",
    "trace-symbol",
    "workspaces",
];

pub(super) fn args_with_default_check<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if should_insert_check(&args) {
        args.insert(1, OsString::from("check"));
    }
    args
}

fn should_insert_check(args: &[OsString]) -> bool {
    let Some(first) = args.get(1).and_then(|arg| arg.to_str()) else {
        return args.len() == 1;
    };
    if matches!(first, "-h" | "--help" | "-V" | "--version") {
        return false;
    }
    first_non_flag(args).is_none_or(|candidate| !COMMANDS.contains(&candidate))
}

fn first_non_flag(args: &[OsString]) -> Option<&str> {
    args.iter()
        .skip(1)
        .filter_map(|arg| arg.to_str())
        .find(|arg| !arg.starts_with('-'))
}
