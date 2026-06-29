use crate::output::ReportCommand;

pub(super) fn report_command_from_name(name: &str) -> ReportCommand {
    match name {
        "check" => ReportCommand::Check,
        "audit" => ReportCommand::Audit,
        "dead-code" => ReportCommand::DeadCode,
        "cycles" => ReportCommand::Cycles,
        "dupes" => ReportCommand::Dupes,
        "health" => ReportCommand::Health,
        "flags" => ReportCommand::Flags,
        "security" => ReportCommand::Security,
        "trace" | "trace-symbol" => ReportCommand::TraceSymbol,
        "trace-file" => ReportCommand::TraceFile,
        "trace-dependency" => ReportCommand::TraceDependency,
        "trace-clone" => ReportCommand::TraceClone,
        "inspect" => ReportCommand::Inspect,
        _ => unreachable!("clap rejects unknown subcommands"),
    }
}
