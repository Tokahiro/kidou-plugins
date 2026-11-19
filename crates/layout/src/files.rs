//! A layout's folder, read into memory once: what the check, the compiler and the hash see.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::{DRAWN, FOLDER};

/// The largest file of a layout, in bytes.
pub const MAX_FILE: u64 = 8 << 20;
/// The most bytes all files of a layout hold together.
pub const MAX_TOTAL: u64 = 32 << 20;
/// The most files a layout holds.
pub const MAX_FILES: usize = 256;
/// The longest side of an image of a layout as Slint reads it, in pixels: one of 30 000 by 30 000 decodes
/// to 3.6 GB.
pub const IMAGE_SIDE_MAX: u32 = 4096;
/// The deepest a file lies under the layout's folder, the folder itself counted as 1.
const DEPTH_MAX: usize = 4;
/// A project file, which Slint's compiler would read settings from: a layout has none.
const PROJECT_FILE: &str = "slint-project.json";

/// A layout's files by name: the path under the layout's folder, its parts joined by `/`; and the
/// PNG `kidouplug` drew of each of its SVGs, by the SVG's name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Files {
    files: BTreeMap<String, Vec<u8>>,
    drawn: BTreeMap<String, Vec<u8>>,
}

impl Files {
    /// Every file under `folder`, or why the layout is refused: a link or a junction anywhere
    /// in it, a name Kidō does not take, a project file, or more than the caps allow.
    pub fn read(folder: &Path) -> Result<Self, String> {
        let mut files = Self::default();
        let mut total = 0;
        files.read_into(folder, "", 1, &mut total)?;
        Ok(files)
    }

    /// The files of `entries`, each as (name, bytes), held to the same names and caps as
    /// [`Files::read`]: the tests' and the fuzzer's packages.
    pub fn of<'a>(entries: impl IntoIterator<Item = (&'a str, &'a [u8])>) -> Result<Self, String> {
        let mut files = Self::default();
        let mut total = 0;
        for (name, bytes) in entries {
            let folders: Vec<&str> = name.split('/').collect();
            let folders = &folders[..folders.len() - 1];
            if folders.len() >= DEPTH_MAX {
                return Err(format!("{name}: folders deeper than {DEPTH_MAX}"));
            }
            for part in folders {
                check_part(part)?;
            }
            files.add(name.to_owned(), bytes.to_vec(), &mut total)?;
        }
        Ok(files)
    }

    fn read_into(
        &mut self,
        folder: &Path,
        prefix: &str,
        depth: usize,
        total: &mut u64,
    ) -> Result<(), String> {
        if depth > DEPTH_MAX {
            return Err(format!("{prefix}: folders deeper than {DEPTH_MAX}"));
        }
        let at = |error: std::io::Error| format!("{}: {error}", folder.display());
        let mut entries = fs::read_dir(folder)
            .map_err(at)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(at)?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let Some(part) = entry.file_name().to_str().map(str::to_owned) else {
                return Err(format!("{prefix}: a name that is no text"));
            };
            check_part(&part)?;
            let name = format!("{prefix}{part}");
            // `symlink_metadata` does not follow a link; on Windows it reports a junction as one.
            let kind = fs::symlink_metadata(entry.path()).map_err(at)?.file_type();
            if kind.is_symlink() {
                return Err(format!("{name}: a link"));
            }
            if kind.is_dir() {
                self.read_into(&entry.path(), &format!("{name}/"), depth + 1, total)?;
            } else if kind.is_file() {
                let mut bytes = Vec::new();
                fs::File::open(entry.path())
                    .and_then(|file| file.take(MAX_FILE + 1).read_to_end(&mut bytes))
                    .map_err(at)?;
                self.add(name, bytes, total)?;
            } else {
                return Err(format!("{name}: neither a file nor a folder"));
            }
        }
        Ok(())
    }

    fn add(&mut self, name: String, bytes: Vec<u8>, total: &mut u64) -> Result<(), String> {
        let last = name.rsplit('/').next().unwrap_or_default();
        check_part(last)?;
        if last == PROJECT_FILE {
            return Err(format!("{name}: a layout has no project file"));
        }
        if crate::served(&name).is_some() {
            return Err(format!("{name}: Kidō serves this file itself"));
        }
        let size = bytes.len() as u64;
        if size > MAX_FILE {
            return Err(format!("{name}: larger than {MAX_FILE} bytes"));
        }
        *total += size;
        if *total > MAX_TOTAL {
            return Err(format!("the files hold more than {MAX_TOTAL} bytes"));
        }
        self.files.insert(name, bytes);
        if self.files.len() > MAX_FILES {
            return Err(format!("more than {MAX_FILES} files"));
        }
        Ok(())
    }

    /// The file `name`, if the layout holds it.
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name).map(Vec::as_slice)
    }

    /// The names of the files, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// The layout of an installed plugin, whose folder is `plugin`: its files under [`FOLDER`]
    /// and the PNGs under [`DRAWN`], one for each SVG and no other, each named as its SVG with
    /// `.png` added.
    pub fn installed(plugin: &Path) -> Result<Self, String> {
        let mut files = Self::read(&plugin.join(FOLDER))?;
        let folder = plugin.join(DRAWN);
        let drawn = if folder.exists() || files.svgs().next().is_some() {
            Self::read(&folder)?.files
        } else {
            BTreeMap::new()
        };
        for (name, png) in drawn {
            let svg = name
                .strip_suffix(".png")
                .filter(|svg| files.svgs().any(|known| known == *svg))
                .ok_or_else(|| format!("{DRAWN}/{name}: no PNG of an SVG of the layout"))?;
            files.drawn.insert(svg.to_owned(), png);
        }
        if let Some(svg) = files.svgs().find(|svg| !files.drawn.contains_key(*svg)) {
            return Err(format!("{svg}: not drawn as a PNG"));
        }
        Ok(files)
    }

    /// Draws each SVG of the layout as a PNG with `draw`, which takes the SVG's name and bytes.
    pub fn draw(
        &mut self,
        mut draw: impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
    ) -> Result<(), String> {
        let mut drawn = BTreeMap::new();
        let mut total = 0;
        for svg in self.svgs() {
            let png = draw(svg, self.get(svg).unwrap_or_default())
                .map_err(|why| format!("{svg}: {why}"))?;
            if png.len() as u64 > MAX_FILE {
                return Err(format!(
                    "{svg}: drawn as a PNG larger than {MAX_FILE} bytes"
                ));
            }
            total += png.len() as u64;
            if total > MAX_TOTAL {
                return Err(format!(
                    "the PNGs drawn of the SVGs hold more than {MAX_TOTAL} bytes"
                ));
            }
            drawn.insert(svg.to_owned(), png);
        }
        self.drawn = drawn;
        Ok(())
    }

    /// The PNG drawn of each SVG, as (the name of its file under [`DRAWN`], its bytes).
    pub fn drawn(&self) -> impl Iterator<Item = (String, &[u8])> {
        self.drawn
            .iter()
            .map(|(svg, png)| (format!("{svg}.png"), png.as_slice()))
    }

    /// The PNG drawn of the SVG `svg`.
    pub(crate) fn drawn_of(&self, svg: &str) -> Option<&[u8]> {
        self.drawn.get(svg).map(Vec::as_slice)
    }

    /// The names of the layout's SVGs.
    fn svgs(&self) -> impl Iterator<Item = &str> {
        self.names()
            .filter(|name| kind_of(name).as_deref() == Some("svg"))
    }

    /// The SHA-256, in lowercase hex, of every file's name, length and bytes, in name order,
    /// then of each drawn PNG's the same way under a name no file has: a file or a drawn PNG
    /// added, removed, renamed or changed changes it.
    pub fn sum(&self) -> String {
        use std::fmt::Write;

        let mut hash = Sha256::new();
        let drawn = self
            .drawn
            .iter()
            .map(|(svg, png)| (format!("\0{DRAWN}/{svg}"), png));
        let files = self.files.iter().map(|(name, bytes)| (name.clone(), bytes));
        for (name, bytes) in files.chain(drawn) {
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        let mut sum = String::with_capacity(64);
        for byte in hash.finalize() {
            let _ = write!(sum, "{byte:02x}");
        }
        sum
    }
}

/// Refuses a part of a file's path that is empty, starts with a dot or holds anything but ASCII
/// letters, digits, `-`, `_` and `.`: no `..`, no drive, no hidden file.
fn check_part(part: &str) -> Result<(), String> {
    let fits = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    if part.is_empty() || part.starts_with('.') || !part.chars().all(fits) {
        return Err(format!(
            "{part:?}: a name of a layout's file has ASCII letters, digits, '-', '_' and '.', \
             and starts with no '.'"
        ));
    }
    Ok(())
}

/// The kind of the file `name`, by its ending, in lowercase.
fn kind_of(name: &str) -> Option<String> {
    name.rsplit_once('.')
        .map(|(_, kind)| kind.to_ascii_lowercase())
}

/// An image of a layout as Slint reads it: a PNG or a JPEG as it is, an SVG as the PNG
/// `kidouplug` drew of it.
pub(crate) struct Shown<'a> {
    /// The media type of `bytes`.
    pub kind: &'static str,
    pub bytes: &'a [u8],
    /// The pixels `bytes` decode to.
    pub pixels: u64,
}

/// The image `name` of the layout `files` as Slint reads it, or why it is refused: a kind other
/// than PNG, JPEG or SVG, a size it does not tell or larger than [`IMAGE_SIDE_MAX`] at a side, or
/// an animated PNG. Slint draws an SVG at the size of the element that shows it, which no file
/// tells, so Slint gets the PNG drawn of it.
pub(crate) fn shown<'a>(name: &str, files: &'a Files) -> Result<Shown<'a>, String> {
    let at = |why: &str| format!("{name}: {why}");
    let bytes = files
        .get(name)
        .ok_or_else(|| at("an image that is not in the layout"))?;
    let (kind, bytes, size) = match kind_of(name).as_deref() {
        Some("png") => ("image/png", bytes, png_size(bytes)),
        Some("jpg" | "jpeg") => (
            "image/jpeg",
            bytes,
            jpeg_size(bytes).ok_or("its size cannot be read"),
        ),
        Some("svg") => {
            let png = files
                .drawn_of(name)
                .ok_or_else(|| at("not drawn as a PNG"))?;
            ("image/png", png, png_size(png))
        }
        _ => return Err(at("an image is a PNG, a JPEG or an SVG")),
    };
    let pixels = match size.map_err(at)? {
        (width, height) if width <= IMAGE_SIDE_MAX && height <= IMAGE_SIDE_MAX => {
            u64::from(width) * u64::from(height)
        }
        (width, height) => {
            return Err(format!(
                "{name}: {width} by {height} pixels, larger than {IMAGE_SIDE_MAX} at a side"
            ));
        }
    };
    Ok(Shown {
        kind,
        bytes,
        pixels,
    })
}

/// The SVG `bytes` drawn by Slint's SVG reader as a PNG, at twice its own size and at most
/// [`IMAGE_SIDE_MAX`] at a side, or why it is refused: no UTF-8 XML, a DTD, an `image` or
/// `feImage`, or an `href` to anything but a part of the same file, as Slint's SVG reader (usvg)
/// would read a file from the disk. `kidouplug` runs it in a process of its own, within a time
/// and a memory limit, as other SVGs can draw for minutes or ask for gigabytes.
pub fn draw_svg(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    use i_slint_core::graphics::{Image, IntSize, image_to_rgba8_with_target_size};

    svg_refusal(bytes).map_or(Ok(()), Err)?;
    let image = Image::load_from_svg_data(bytes).map_err(|_| "an SVG Slint cannot read")?;
    let size = image.size();
    let (width, height) = drawn_size(size.width, size.height).ok_or("an SVG of no size")?;
    let pixels = image_to_rgba8_with_target_size(&image, IntSize::new(width, height))
        .ok_or("an SVG Slint cannot draw")?;
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, pixels.width(), pixels.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels.as_bytes()))
        .map_err(|_| "an SVG Slint cannot draw")?;
    Ok(png)
}

/// Twice `width` by `height`, scaled down to at most [`IMAGE_SIDE_MAX`] at a side; `None` for an
/// empty size.
fn drawn_size(width: u32, height: u32) -> Option<(u32, u32)> {
    let (width, height) = (u64::from(width) * 2, u64::from(height) * 2);
    let longest = width.max(height);
    if width == 0 || height == 0 {
        return None;
    }
    let side = |side: u64| {
        let scaled = if longest > u64::from(IMAGE_SIDE_MAX) {
            side * u64::from(IMAGE_SIDE_MAX) / longest
        } else {
            side
        };
        u32::try_from(scaled.max(1)).ok()
    };
    Some((side(width)?, side(height)?))
}

/// Why [`draw_svg`] refuses an SVG, if it does.
fn svg_refusal(bytes: &[u8]) -> Option<&'static str> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Some("an SVG of no UTF-8 text");
    };
    // roxmltree refuses a DTD unless it is allowed.
    let Ok(document) = roxmltree::Document::parse(text) else {
        return Some("an SVG that is no XML without a DTD");
    };
    for element in document.descendants().filter(roxmltree::Node::is_element) {
        // usvg reads the file an `image` or `feImage` names, even one whose name starts with `#`.
        if matches!(element.tag_name().name(), "image" | "feImage") {
            return Some("an SVG with an image");
        }
        let links_out = element
            .attributes()
            .any(|attribute| attribute.name() == "href" && !attribute.value().starts_with('#'));
        if links_out {
            return Some("an SVG that links to anything but a part of itself");
        }
    }
    None
}

/// A PNG's width and height, from its header, where it is no animated PNG: Slint plays one,
/// which keeps Home drawing.
fn png_size(bytes: &[u8]) -> Result<(u32, u32), &'static str> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    let word = |at: usize| {
        let word = bytes.get(at..at.checked_add(4)?)?;
        Some(u32::from_be_bytes(word.try_into().ok()?))
    };
    if !bytes.starts_with(SIGNATURE) || bytes.get(12..16) != Some(b"IHDR") {
        return Err("its size cannot be read");
    }
    let size = word(16).zip(word(20)).ok_or("its size cannot be read")?;
    let mut at = SIGNATURE.len();
    loop {
        let length = word(at).ok_or("a PNG with no image data")?;
        match bytes.get(at + 4..at + 8) {
            Some(b"acTL") => return Err("an animated PNG"),
            Some(b"IDAT") => return Ok(size),
            Some(_) => {}
            None => return Err("a PNG with no image data"),
        }
        // The chunk's length, type, data and check sum.
        at = usize::try_from(length)
            .ok()
            .and_then(|length| at.checked_add(12)?.checked_add(length))
            .ok_or("a PNG with no image data")?;
    }
}

/// A JPEG's width and height, from its one frame header (SOF0 to SOF15 but DHT, JPG and DAC)
/// before the scan. Fill bytes (`FF`) before a marker are skipped, as decoders do; anything else
/// a decoder would skip or read another way, or a second frame header, gives none.
fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut size = None;
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xff {
            return None;
        }
        while *bytes.get(at + 1)? == 0xff {
            at += 1;
        }
        let marker = *bytes.get(at + 1)?;
        if marker == 0xda {
            return size;
        }
        if matches!(marker, 0x00 | 0x01 | 0xd0..=0xd9) {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]));
        if length < 2 {
            return None;
        }
        if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            if size.is_some() {
                return None;
            }
            let height = u16::from_be_bytes([*bytes.get(at + 5)?, *bytes.get(at + 6)?]);
            let width = u16::from_be_bytes([*bytes.get(at + 7)?, *bytes.get(at + 8)?]);
            size = Some((u32::from(width), u32::from(height)));
        }
        at += 2 + length;
    }
}
