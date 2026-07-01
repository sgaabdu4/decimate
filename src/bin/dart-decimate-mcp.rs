use std::process::ExitCode;

fn main() -> ExitCode {
    match dart_decimate::mcp::run_stdio() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dart-decimate-mcp: {error}");
            ExitCode::from(1)
        }
    }
}
