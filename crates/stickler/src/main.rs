//! Runs one tool, or a check command.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.split_first() {
        Some((&"lint", rest)) => stickler_lint::main(rest),
        Some((&"style", rest)) => stickler_style::main(rest),
        Some((&"simcheck", rest)) => stickler_simcheck::main(rest),
        _ => stickler_check::main(&args),
    }
}
