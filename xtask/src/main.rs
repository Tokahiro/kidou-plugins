use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "\
USAGE:
    cargo xtask check    Hold the list, the packages and the sources to each other, build
                         each plugin with code, and run Kidō's check on each Home layout
    cargo xtask pack     Pack each listed source whose version has no package yet, and list it
    cargo xtask draw     Draw the SVG of the standard input as a PNG to the standard output, as
                         the check does in a process of its own
";

fn main() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let done = match std::env::args().nth(1).as_deref() {
        Some("check") => xtask::check(&root).and_then(|mut wrong| {
            let drawer = std::env::current_exe().map_err(|error| error.to_string())?;
            wrong.extend(xtask::layouts(&root, &drawer)?);
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
        Some("draw") => kidou_layout::draw(std::io::stdin().lock(), std::io::stdout().lock())
            .map(|()| Vec::new()),
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
