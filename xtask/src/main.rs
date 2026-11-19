use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
USAGE:
    cargo xtask check    Hold the list, the packages and the sources to each other
    cargo xtask pack     Pack each source whose version has no package yet, and list it
";

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let done = match std::env::args().nth(1).as_deref() {
        Some("check") => xtask::check(&root).and_then(|wrong| {
            if wrong.is_empty() {
                Ok(Vec::new())
            } else {
                Err(wrong.join("\n"))
            }
        }),
        Some("pack") => xtask::pack_new(&root),
        _ => Err(USAGE.to_owned()),
    };
    match done {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
