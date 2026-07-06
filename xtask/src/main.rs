use std::process::ExitCode;

fn main() -> ExitCode {
    xtask::commands::run_from_env()
}
