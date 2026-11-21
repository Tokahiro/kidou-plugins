//! Home layouts by others.
//!
//! A layout package holds a folder [`FOLDER`] whose [`ROOT`] exports `Layout`, the root
//! [`CONTRACT`] names. [`Files::read`] reads that folder into memory once; `kidouplug` draws
//! each SVG of it as a PNG and runs [`check`] and [`compile`] on that copy ([`checked`]), installs
//! the drawn PNGs in the folder [`DRAWN`] and writes a [`Record`] of both; Home reads them again
//! ([`Files::installed`]), compares them with the record and compiles that copy, so Home never
//! reads an SVG. The record covers [`FOLDER`] and [`DRAWN`] only, as Home reads no other file
//! of the package for the layout. Every file
//! Slint's compiler or interpreter asks for comes from the copy or is refused, so neither reads
//! the disk.

mod check;
mod compile;
mod draw;
mod embed;
mod files;
mod record;
mod source;

pub use check::{ANIMATION_MAX_MS, PIXELS_MAX, check};
pub use compile::{Refused, compile, load};
pub use draw::{MEMORY_MAX, TIME_MAX, TOTAL_TIME_MAX, checked, draw};
pub use embed::{
    CardSize, Embedded, EmptyWords, Fact, Given, Hero, Place, Placed, Theme, Tile, TitleLength,
    Zone,
};
pub use files::{Files, IMAGE_SIDE_MAX, MAX_FILE, MAX_FILES, MAX_TOTAL, draw_svg};
pub use record::{Mismatch, Record, record_path};
pub use slint_interpreter::ComponentDefinition;

/// The folder of a plugin package that holds its layout.
pub const FOLDER: &str = "layout";

/// The folder of an installed plugin that holds the PNG `kidouplug` drew of each SVG of its
/// layout, beside [`FOLDER`].
pub const DRAWN: &str = "drawn";

/// The layout's file that exports `Layout`.
pub const ROOT: &str = "layout.slint";

/// The layout contract a layout imports by its name, as (name, text): Home serves it from
/// memory, whatever the package holds.
pub const CONTRACT: (&str, &str) = (
    "kidou-layout-3.slint",
    include_str!("../contract/kidou-layout-3.slint"),
);

/// The version of the contract [`CONTRACT`] and [`PARTS`] hold: the latest a layout's `api` may
/// name. A name they export more is a new minor version; from Kidō's first release on, a change
/// that breaks a layout is a new major version (`docs/layouts.md`, "Contract Versions").
pub const VERSION: &str = "3.1.0";

/// Kidō's parts a layout may import beside [`CONTRACT`], by their name, as (name, text): Home
/// serves them from memory too.
pub const PARTS: (&str, &str) = (
    "kidou-layout-parts-3.slint",
    include_str!("../contract/kidou-layout-parts-3.slint"),
);

/// The cover [`PARTS`] exports, by its name, as (name, text): Home serves it from memory too.
const COVER: (&str, &str) = (
    "kidou-layout-cover-3.slint",
    include_str!("../contract/kidou-layout-cover-3.slint"),
);

/// The text of `name`, where it is a file Home serves itself: [`CONTRACT`], [`PARTS`] or
/// [`COVER`].
fn served(name: &str) -> Option<&'static str> {
    [CONTRACT, PARTS, COVER]
        .into_iter()
        .find(|(served, _)| *served == name)
        .map(|(_, text)| text)
}

/// The version of Kidō a [`Record`] names: the full name of Kidō's package, `package`, which
/// changes with each update, or this build's version where Kidō runs unpackaged.
pub fn version(package: Option<String>) -> String {
    package.unwrap_or_else(|| format!("unpackaged {}", env!("CARGO_PKG_VERSION")))
}

/// Runs `future` to its end on this thread. The compiler's futures are only asynchronous where
/// a loader's are, and [`source`]'s never wait.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}
