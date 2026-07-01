use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(u8::try_from(dart_decimate::cli::run_from_env()).unwrap_or(1))
}
