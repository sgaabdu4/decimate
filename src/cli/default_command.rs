use std::ffi::OsString;

const COMMANDS: &[&str] = &[
    "audit",
    "check",
    "ci",
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
    "html",
    "hooks",
    "impact",
    "init",
    "inspect",
    "license",
    "list",
    "migrate",
    "report-schema",
    "review",
    "rule-pack-schema",
    "schema",
    "security",
    "setup-hooks",
    "telemetry",
    "trace",
    "trace-clone",
    "trace-dependency",
    "trace-file",
    "trace-symbol",
    "watch",
    "workspaces",
    "human",
    "json",
];

pub(super) fn args_with_default_check<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if let Some(expanded) = args_with_output_alias(&args) {
        return expanded;
    }
    if should_insert_check(&args) {
        args.insert(1, OsString::from("check"));
    }
    args
}

fn args_with_output_alias(args: &[OsString]) -> Option<Vec<OsString>> {
    let alias = args.get(1)?.to_str()?;
    if args.iter().skip(2).any(is_help_arg) {
        return None;
    }
    let format = match alias {
        "human" => "human",
        "json" => "json",
        "html" => "html",
        _ => return None,
    };
    let mut expanded = Vec::with_capacity(args.len() + 3);
    expanded.push(args[0].clone());
    expanded.push(OsString::from("check"));
    let mut stdout_html = false;
    let mut iter = args.iter().skip(2);
    while let Some(arg) = iter.next() {
        if alias == "html" && arg == "--stdout" {
            stdout_html = true;
            continue;
        }
        if arg == "--format" {
            iter.next();
            continue;
        }
        if arg
            .to_str()
            .is_some_and(|value| value.starts_with("--format="))
        {
            continue;
        }
        expanded.push(arg.clone());
    }
    expanded.push(OsString::from("--format"));
    expanded.push(OsString::from(format));
    if alias == "html" && !stdout_html {
        expanded.push(OsString::from("--open"));
    }
    Some(expanded)
}

fn is_help_arg(arg: &OsString) -> bool {
    arg.to_str()
        .is_some_and(|value| matches!(value, "-h" | "--help"))
}

fn should_insert_check(args: &[OsString]) -> bool {
    let Some(first) = args.get(1).and_then(|arg| arg.to_str()) else {
        return args.len() == 1;
    };
    if matches!(first, "-h" | "--help" | "-V" | "--version") {
        return false;
    }
    !COMMANDS.contains(&first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(items: &[&str]) -> Vec<OsString> {
        items.iter().map(OsString::from).collect()
    }

    #[test]
    fn output_aliases_expand_to_check_formats() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "human", "app"]),
            values(&["dart-decimate", "check", "app", "--format", "human"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "json", "app", "--format", "human"]),
            values(&["dart-decimate", "check", "app", "--format", "json"])
        );
    }

    #[test]
    fn html_alias_opens_unless_stdout_is_requested() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "html", "app"]),
            values(&[
                "dart-decimate",
                "check",
                "app",
                "--format",
                "html",
                "--open",
            ])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "html", "app", "--stdout"]),
            values(&["dart-decimate", "check", "app", "--format", "html"])
        );
    }

    #[test]
    fn leading_format_values_do_not_block_default_check() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "--format", "json"]),
            values(&["dart-decimate", "check", "--format", "json"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "--format", "html", "app"]),
            values(&["dart-decimate", "check", "--format", "html", "app"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "--format=json", "app"]),
            values(&["dart-decimate", "check", "--format=json", "app"])
        );
    }
}
