//! SteamGridDB's API: its one host, and the requests made of it. Paths only are joined to the
//! base, so no request can go to another host.

use std::fmt::Write as _;

/// The API's base; every request starts with it.
pub const BASE: &str = "https://www.steamgriddb.com/api/v2";

/// The API's host, the only one that is sent the key.
pub const HOST: &str = "www.steamgriddb.com";

/// The platforms whose own ids the art endpoints take (`/grids/<platform>/<id>`), as the API's
/// documentation lists them. A game of another platform is first found by its id through
/// `/games/<platform>/<id>`, else by its title.
pub const ART_PLATFORMS: &[&str] = &[
    "steam",
    "origin",
    "egs",
    "bnet",
    "uplay",
    "flashpoint",
    "eshop",
];

/// The platform at SteamGridDB of each store whose ids it knows, by the store's id.
pub const PLATFORMS: &[(&str, &str)] = &[("steam", "steam"), ("gog", "gog")];

/// The platform word for SteamGridDB's own game ids.
pub const OWN: &str = "game";

/// A kind of art SteamGridDB serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Square,
    Cover,
    Hero,
    Logo,
}

/// A request of the API: the path after [`BASE`] and its query, without the key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApiRequest {
    pub path: String,
    pub query: Vec<(&'static str, String)>,
}

impl ApiRequest {
    /// The whole address: [`BASE`], the path, and the query with its values percent-encoded.
    pub fn url(&self) -> String {
        let mut url = String::from(BASE);
        url.push_str(&self.path);
        for (at, (name, value)) in self.query.iter().enumerate() {
            url.push(if at == 0 { '?' } else { '&' });
            url.push_str(name);
            url.push('=');
            encode_into(&mut url, value, b",");
        }
        url
    }
}

/// The sizes SteamGridDB is asked for, as (width, height), the preferred first. Logos have
/// none: SteamGridDB's logos come in any size and take no `dimensions` filter, and Kidō fits any
/// of them into the logo box.
pub fn dimensions(kind: Kind) -> &'static [(u32, u32)] {
    match kind {
        Kind::Square => &[(512, 512), (1024, 1024)],
        Kind::Cover => &[(600, 900), (660, 930)],
        // 3840 x 1240 would cost four times the memory to decode, for detail the window drops.
        Kind::Hero => &[(1920, 620)],
        Kind::Logo => &[],
    }
}

/// The request for art of `kind` of the game `id` on `platform` (`"steam"`, `"gog"`, or
/// `"game"` for SteamGridDB's own id).
///
/// Only static pictures are asked for, none marked unsafe or a joke, in the sizes of
/// [`dimensions`].
pub fn art(kind: Kind, platform: &str, id: &str) -> ApiRequest {
    let endpoint = match kind {
        Kind::Square | Kind::Cover => "grids",
        Kind::Hero => "heroes",
        Kind::Logo => "logos",
    };
    let sizes = dimensions(kind)
        .iter()
        .map(|(width, height)| format!("{width}x{height}"))
        .collect::<Vec<_>>()
        .join(",");
    let mut query = Vec::with_capacity(4);
    if !sizes.is_empty() {
        query.push(("dimensions", sizes));
    }
    query.extend([
        ("types", "static".to_owned()),
        ("nsfw", "false".to_owned()),
        ("humor", "false".to_owned()),
    ]);
    ApiRequest {
        path: path(&[endpoint, platform, id]),
        query,
    }
}

/// The request for the game SteamGridDB knows as `id` on `platform`, which answers with its own
/// id.
pub fn game(platform: &str, id: &str) -> ApiRequest {
    ApiRequest {
        path: path(&["games", platform, id]),
        query: Vec::new(),
    }
}

/// The request for games whose names match `title`, searched for as [`search_term`] gives it.
pub fn search(title: &str) -> ApiRequest {
    ApiRequest {
        path: path(&["search", "autocomplete", &search_term(title)]),
        query: Vec::new(),
    }
}

/// `title` as it is searched for: without trademark signs, its white space made single. Empty
/// when nothing is left to search for.
pub fn search_term(title: &str) -> String {
    title
        .split(|c: char| c.is_whitespace() || MARKS.contains(&c))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Trademark and copyright signs, which some stores put in titles and SteamGridDB leaves out.
pub(crate) const MARKS: &[char] = &['\u{2122}', '\u{00ae}', '\u{00a9}', '\u{2120}'];

/// `/` and each segment percent-encoded, so a segment cannot add a part to the path, a query or
/// a fragment. The dots of a segment of dots alone are encoded too, so that it cannot step up.
fn path(segments: &[&str]) -> String {
    let mut path = String::new();
    for segment in segments {
        path.push('/');
        if segment.bytes().all(|byte| byte == b'.') {
            path.push_str(&"%2E".repeat(segment.len()));
        } else {
            encode_into(&mut path, segment, b"");
        }
    }
    path
}

/// Appends `text` with every byte percent-encoded except ASCII letters and digits, `-._~` and
/// those in `keep`.
fn encode_into(out: &mut String, text: &str, keep: &[u8]) {
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) || keep.contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
}

#[cfg(test)]
mod tests;
