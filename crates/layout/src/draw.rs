//! A layout checked as an install checks it ([`checked`]): each SVG drawn as a PNG in a process
//! of its own, then [`check`](crate::check) and [`compile`](crate::compile). An SVG can draw for
//! minutes or ask for gigabytes, which a check of its elements does not foresee, so each process
//! has [`TIME_MAX`], all of a layout's SVGs together [`TOTAL_TIME_MAX`], and past either the
//! layout is refused. The program that draws runs [`draw`]. Kidō's own sets [`MEMORY_MAX`] for
//! itself first, on Windows; no limit of memory is set here.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How often [`in_child`] looks whether the process that closed its output has ended.
const ENDED_EVERY: Duration = Duration::from_millis(10);

use crate::{Files, MAX_FILE};

/// The longest one SVG may draw.
pub const TIME_MAX: Duration = Duration::from_secs(10);

/// The longest all SVGs of a layout may draw together.
pub const TOTAL_TIME_MAX: Duration = Duration::from_secs(60);

/// The most memory the process that draws one SVG may commit, in bytes: the PNG of 4096 by 4096
/// pixels takes 64 MiB, its layers a few times that.
pub const MEMORY_MAX: usize = 1 << 30;

/// The layout `files` with its SVGs drawn, where it passes the check and compiles, or why it is
/// refused. `drawer` is a program whose command `draw` runs [`draw`] on its standard input and
/// output: it draws each SVG, within [`TIME_MAX`] and [`TOTAL_TIME_MAX`].
pub fn checked(mut files: Files, drawer: &Path) -> Result<Files, Vec<String>> {
    let start = Instant::now();
    files
        .draw(|_, svg| {
            let left = TOTAL_TIME_MAX.saturating_sub(start.elapsed());
            in_child(drawer, svg, left)
        })
        .map_err(|why| vec![why])?;
    crate::check(&files)?;
    crate::compile(&files)?;
    Ok(files)
}

/// The SVG of `input` drawn as a PNG to `output`: what a drawer's command `draw` runs.
pub fn draw(input: impl Read, mut output: impl Write) -> Result<(), String> {
    let mut svg = Vec::new();
    input
        .take(MAX_FILE + 1)
        .read_to_end(&mut svg)
        .map_err(|error| error.to_string())?;
    let png = crate::draw_svg(&svg)?;
    output
        .write_all(&png)
        .and_then(|()| output.flush())
        .map_err(|error| error.to_string())
}

/// The SVG `svg` drawn as a PNG by `drawer draw`, or why not: refused, larger than
/// [`MAX_FILE`], past `time` (at most [`TIME_MAX`]), or ended by its own limits.
fn in_child(drawer: &Path, svg: &[u8], time: Duration) -> Result<Vec<u8>, String> {
    let mut child = Command::new(drawer)
        .arg("draw")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{}: {error}", drawer.display()))?;
    let png = answer(&mut child, svg.to_vec(), time.min(TIME_MAX));
    if png.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    png
}

/// The PNG the drawing process `child` writes of `svg`, where it ends well within `time`.
fn answer(child: &mut Child, svg: Vec<u8>, time: Duration) -> Result<Vec<u8>, String> {
    let (Some(mut stdin), Some(stdout), Some(mut stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err("the drawing process has no pipes".to_owned());
    };
    // Each pipe on a thread of its own, so that no full pipe stops the process.
    std::thread::spawn(move || {
        let _ = stdin.write_all(&svg);
    });
    let why = std::thread::spawn(move || {
        let mut why = Vec::new();
        let _ = (&mut stderr).take(4096).read_to_end(&mut why);
        String::from_utf8_lossy(&why).into_owned()
    });
    let (sent, read) = mpsc::channel();
    std::thread::spawn(move || {
        let mut png = Vec::new();
        let result = stdout.take(MAX_FILE + 1).read_to_end(&mut png).map(|_| png);
        let _ = sent.send(result);
    });
    let start = Instant::now();
    let late = || {
        format!(
            "it draws longer than its time, {} s for one SVG and {} s for all",
            TIME_MAX.as_secs(),
            TOTAL_TIME_MAX.as_secs()
        )
    };
    let png = read
        .recv_timeout(time)
        .map_err(|_| late())?
        .map_err(|error| error.to_string())?;
    if u64::try_from(png.len()).is_ok_and(|size| size > MAX_FILE) {
        return Err(format!("drawn as a PNG larger than {MAX_FILE} bytes"));
    }
    let status = loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(status) => break status,
            None if start.elapsed() < time => std::thread::sleep(ENDED_EVERY),
            None => return Err(late()),
        }
    };
    if !status.success() {
        let why = why.join().unwrap_or_default();
        let why = why.trim();
        return Err(if why.is_empty() {
            format!("the drawing ended with {status}, past the memory limit or broken")
        } else {
            why.to_owned()
        });
    }
    Ok(png)
}
