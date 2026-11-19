//! A layout by others: read into memory, checked, compiled and matched with its record, from
//! the sample in `good/` and from layouts written to misbehave.

mod check;
mod contract;
mod files;

use std::path::{Path, PathBuf};

use kidou_layout::Files;

/// The sample layout that passes, as its folder holds it.
fn good() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/it/good")
}

/// A layout of the contract's root with `body` inside it, and `extra` files beside it, each SVG
/// drawn as `kidouplug` draws it.
fn layout(body: &str, extra: &[(&str, &[u8])]) -> Files {
    let contract = kidou_layout::CONTRACT.0;
    let text = format!(
        r#"import {{
    LayoutTile, LayoutHero, LayoutEmptyWords, LayoutTheme, LayoutZone, LayoutPlace,
}} from "{contract}";
export component Layout {{
    in property <[LayoutTile]> tiles;
    in property <int> focused;
    in property <LayoutZone> zone;
    in property <image> stage;
    in property <LayoutHero> hero;
    in property <bool> empty;
    in property <LayoutEmptyWords> empty-words;
    in property <[string]> tabs;
    in property <int> tab;
    in property <LayoutTheme> theme;
    out property <length> actions-x;
    out property <length> actions-y;
    out property <LayoutPlace> focused-place;
    callback activated(int);
    callback tab-tapped(int);
    {body}
}}
"#
    );
    let mut entries = vec![(kidou_layout::ROOT, text.as_bytes())];
    entries.extend_from_slice(extra);
    drawn(Files::of(entries).expect("the test's files"))
}

/// `files` with each SVG drawn as a PNG, here in the test's own process.
fn drawn(mut files: Files) -> Files {
    files
        .draw(|_, svg| kidou_layout::draw_svg(svg).map_err(str::to_owned))
        .expect("the test's SVGs draw");
    files
}

/// A folder of the test's own, empty, under cargo's target directory.
fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("layout")
        .join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("the test's folder");
    folder
}
