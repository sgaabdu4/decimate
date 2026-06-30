use std::process::ExitCode;

fn main() -> ExitCode {
    match decimate::mcp::run_stdio() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("decimate-mcp: {error}");
            ExitCode::from(1)
        }
    }
}
