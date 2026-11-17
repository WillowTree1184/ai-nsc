use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(ainsc::driver::run() as u8)
}
