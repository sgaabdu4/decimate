use clap::{Arg, ArgAction, Command};

pub(super) fn summary_command(command: Command) -> Command {
    command.arg(summary_arg())
}

fn summary_arg() -> Arg {
    Arg::new("summary")
        .long("summary")
        .help("Show summary counts")
        .action(ArgAction::SetTrue)
}
