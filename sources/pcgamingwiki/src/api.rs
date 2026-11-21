//! The two calls of PCGamingWiki's MediaWiki API the plugin makes, and the answers it reads
//! (<https://www.pcgamingwiki.com/wiki/PCGamingWiki:API>): `idlookup` finds the wiki page of a
//! Steam or GOG id, and `parse` gives a page's wikitext. Neither needs an account; the Cargo
//! tables do, so the plugin reads the page's templates itself.

use std::fmt::Write as _;

use serde::Deserialize;

/// The API's address.
pub const API: &str = "https://www.pcgamingwiki.com/w/api.php";

/// The User-Agent the API asks of an automated client: its own name and where to reach its
/// makers. A generic one can be refused.
pub const AGENT: &str = "Kidou-PCGamingWiki/1.0 (+https://github.com/Tokahiro/kidou-plugins)";

/// The longest page title sent, in bytes. MediaWiki's own limit is 255.
const TITLE_MAX: usize = 255;

/// The `idlookup` field of the store of id `store`: Steam's app id and GOG's product id; none
/// for another store.
pub fn id_field(store: &str) -> Option<&'static str> {
    match store {
        "steam" => Some("steamappid"),
        "gog" => Some("gogid"),
        _ => None,
    }
}

/// The address that looks up the page of the store id `id` under `field`; `None` for an id that
/// is not digits alone, which is never sent.
pub fn id_lookup(field: &str, id: &str) -> Option<String> {
    let digits = !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit());
    digits.then(|| {
        format!("{API}?action=idlookup&field={field}&value={id}&format=json&formatversion=2")
    })
}

/// The address that gives the wikitext of the page `title`, after the redirects that lead from
/// it; `None` for a title MediaWiki takes for no page, which is never sent.
pub fn wikitext(title: &str) -> Option<String> {
    let title = title.trim();
    let usable = !title.is_empty()
        && title.len() <= TITLE_MAX
        && !title
            .chars()
            .any(|c| c.is_control() || "#<>[]{}|".contains(c));
    usable.then(|| {
        format!(
            "{API}?action=parse&page={}&prop=wikitext&redirects=1&format=json&formatversion=2",
            encode(title)
        )
    })
}

/// The address of the page `title` on the wiki, without the scheme, as the credit names it.
pub fn page(title: &str) -> String {
    format!("pcgamingwiki.com/wiki/{}", title.trim().replace(' ', "_"))
}

/// `text` percent-encoded for a query, all but RFC 3986's unreserved characters.
fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[derive(Deserialize)]
struct IdLookupJson {
    #[serde(default)]
    idlookup: Vec<FoundJson>,
}

#[derive(Deserialize)]
struct FoundJson {
    title: PageJson,
}

#[derive(Deserialize)]
struct PageJson {
    #[serde(rename = "Page")]
    page: String,
}

#[derive(Deserialize)]
struct ParseJson {
    parse: Option<ParsedJson>,
    error: Option<ErrorJson>,
}

#[derive(Deserialize)]
struct ParsedJson {
    title: String,
    wikitext: String,
}

#[derive(Deserialize)]
struct ErrorJson {
    #[serde(default)]
    code: String,
}

/// The page title in an answer of `idlookup`; `None` when the wiki knows no page of the id.
pub fn found_page(body: &[u8]) -> Result<Option<String>, String> {
    let answer: IdLookupJson = serde_json::from_slice(body)
        .map_err(|_| "PCGamingWiki's id lookup does not read".to_owned())?;
    Ok((answer.idlookup.into_iter())
        .map(|found| found.title.page)
        .find(|page| !page.trim().is_empty()))
}

/// A wiki page: its title after the redirects, and its wikitext.
#[derive(Debug, PartialEq, Eq)]
pub struct Page {
    pub title: String,
    pub wikitext: String,
}

/// The page in an answer of `parse`; `None` where the wiki has no such page.
pub fn parsed(body: &[u8]) -> Result<Option<Page>, String> {
    let answer: ParseJson =
        serde_json::from_slice(body).map_err(|_| "PCGamingWiki's page does not read".to_owned())?;
    match (answer.parse, answer.error) {
        (Some(parsed), _) => Ok(Some(Page {
            title: parsed.title,
            wikitext: parsed.wikitext,
        })),
        (None, Some(error)) if error.code == "missingtitle" || error.code == "invalidtitle" => {
            Ok(None)
        }
        (None, Some(error)) => Err(format!("PCGamingWiki answered {}", short(&error.code))),
        (None, None) => Err("PCGamingWiki's page holds no wikitext".to_owned()),
    }
}

/// `text` cut to at most 40 characters, for a problem's text.
fn short(text: &str) -> String {
    text.chars().take(40).collect()
}

#[cfg(test)]
mod tests;
