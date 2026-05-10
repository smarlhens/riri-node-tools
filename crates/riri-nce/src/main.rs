use std::process::ExitCode;

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    ExitCode::from(riri_nce::cli::run_cli(argv) as u8)
}
