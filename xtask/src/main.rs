use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
USAGE:
    cargo xtask check    Hold the list, the packages and the sources to each other, build
                         each plugin with code, and run Kidō's check on each Home layout
    cargo xtask pack     Pack each listed source whose version has no package yet, and list it
";

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let done = match std::env::args().nth(1).as_deref() {
        Some("check") => xtask::check(&root).and_then(|mut wrong| {
            wrong.extend(xtask::layouts(&root)?);
            for id in xtask::unlisted(&root)? {
                eprintln!("{id}: not on the list, so not offered and not packed");
            }
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
