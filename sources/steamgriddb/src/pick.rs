//! Which of SteamGridDB's pictures to take: static ones only, none marked unsafe, a joke or
//! flashing, the best scored first, then the most voted, then those of the size preferred.

use std::cmp::Reverse;
use std::collections::HashSet;

use crate::api::{self, Kind};
use crate::json::Image;

/// A picture taken: its kind, its address, and whether it shows the game's title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picked {
    pub kind: Kind,
    pub url: String,
    pub titled: bool,
}

/// The longest address taken.
const URL_MAX: usize = 2048;

/// The candidates among `images` for art of `kind`, the best first: those in one of the sizes
/// [`api::dimensions`] gives, ordered by score, then upvotes, then the size's place in that list;
/// a logo in any size.
pub fn pick(kind: Kind, images: &[Image]) -> Vec<Picked> {
    let sizes = api::dimensions(kind);
    let mut usable = images
        .iter()
        .filter(|image| usable(image))
        .filter_map(|image| {
            if kind == Kind::Logo {
                return Some((image, 0));
            }
            let size = sizes
                .iter()
                .position(|&size| size == (image.width, image.height))?;
            Some((image, size))
        })
        .collect::<Vec<_>>();
    usable.sort_by_key(|&(image, size)| (Reverse(image.score), Reverse(image.upvotes), size));
    let mut seen = HashSet::with_capacity(usable.len());
    let mut picked = Vec::with_capacity(usable.len());
    for (image, _) in usable {
        if !seen.insert(image.url.as_str()) {
            continue;
        }
        picked.push(Picked {
            kind,
            url: image.url.clone(),
            // SteamGridDB's heroes are backgrounds that carry no logo, whatever their style.
            titled: kind != Kind::Hero && shows_title(&image.style),
        });
    }
    picked
}

/// Whether a picture of this style carries the game's title.
pub fn shows_title(style: &str) -> bool {
    !matches!(style, "no_logo" | "blurred")
}

/// A still picture that is safe to show, at an `https` address.
fn usable(image: &Image) -> bool {
    !image.nsfw
        && !image.humor
        && !image.epilepsy
        && still(image)
        && image.url.starts_with("https://")
        && image.url.len() <= URL_MAX
        && !image
            .url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
}

/// Whether the picture is of a format the art pipeline decodes, by its type, else by its name.
fn still(image: &Image) -> bool {
    const STILL: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];
    if image.mime.is_empty() {
        let name = image.url.rsplit('/').next().unwrap_or_default();
        let lower = name.to_ascii_lowercase();
        [".png", ".jpg", ".jpeg", ".webp"]
            .iter()
            .any(|extension| lower.ends_with(extension))
    } else {
        STILL.contains(&image.mime.as_str())
    }
}

#[cfg(test)]
mod tests;
