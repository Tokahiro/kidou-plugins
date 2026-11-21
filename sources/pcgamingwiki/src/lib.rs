//! PCGamingWiki as a source plugin, with the manifest `plugin.toml`: a few facts on a game's page,
//! read from the wiki's page of the game. It needs no key and gives no art.
//!
//! It asks only when the game's page shows, with two requests: the page of the game's Steam or
//! GOG id, then that page's wikitext. A game of another store is asked for by its title. Kidō
//! keeps what it gives, so the wiki is asked about a game once and the facts show offline after.
//! The wiki is small and community run and asks clients to ask little: at most 60 requests a
//! minute, with an agent that names the client.
//!
//! Its answers come from the network, so reading them must not panic (a release build aborts on
//! a panic): no indexing, unwrapping or unchecked arithmetic outside the tests.

#![deny(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::arithmetic_side_effects
    )
)]

mod api;
mod facts;
mod wikitext;

pub use facts::{Language, Row};

/// How long the wiki is left alone after it says it gets too many requests (it then blocks for
/// 60 s), or when it is down.
pub const LATER_BUSY: u32 = 60;

/// How long the wiki is left alone after it refuses a request outright: its firewall blocks some
/// networks, and asking again soon changes nothing.
pub const LATER_REFUSED: u32 = 3600;

/// What Kidō asks of a game.
pub struct Ask<'a> {
    pub store: &'a str,
    pub store_id: &'a str,
    pub title: &'a str,
    /// Whether the game's page shows, which needs the facts.
    pub facts: bool,
    pub language: Language,
}

/// An answer of the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Why the network gave no answer, as the host's `http` says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsent {
    Denied,
    TooLarge,
    Unreachable(String),
    Failed(String),
}

/// HTTPS GETs.
pub trait Get {
    fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<Answer, Unsent>;
}

/// Why there is no answer, as `kidou:source`'s `problem` without `refused`: there is no setting
/// to refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The wiki asks to be left alone for this many seconds.
    Later(u32),
    /// Anything else, for Kidō's log.
    Failed(String),
}

/// The rows of facts the wiki tells for `ask`. Nothing, and no request, while the facts are not
/// wanted; nothing where the wiki has no page of the game.
pub fn about(http: &mut dyn Get, ask: &Ask<'_>) -> Result<Vec<Row>, Problem> {
    if !ask.facts {
        return Ok(Vec::new());
    }
    let title = match api::id_field(ask.store) {
        Some(field) => {
            let Some(url) = api::id_lookup(field, ask.store_id) else {
                return Ok(Vec::new());
            };
            let body = get(http, &url)?;
            let Some(title) = api::found_page(&body).map_err(Problem::Failed)? else {
                return Ok(Vec::new());
            };
            title
        }
        None => ask.title.to_owned(),
    };
    let Some(url) = api::wikitext(&title) else {
        return Ok(Vec::new());
    };
    let body = get(http, &url)?;
    let Some(page) = api::parsed(&body).map_err(Problem::Failed)? else {
        return Ok(Vec::new());
    };
    Ok(facts::rows(&page.title, &page.wikitext, ask.language))
}

/// The body of the answer to a GET of `url`, or why there is none.
fn get(http: &mut dyn Get, url: &str) -> Result<Vec<u8>, Problem> {
    let answer = http
        .get(
            url,
            &[("User-Agent", api::AGENT), ("Accept", "application/json")],
        )
        .map_err(|unsent| {
            Problem::Failed(match unsent {
                Unsent::Denied => "Kidō denied the request".to_owned(),
                Unsent::TooLarge => "the answer is larger than Kidō reads".to_owned(),
                Unsent::Unreachable(why) => format!("no network: {why}"),
                Unsent::Failed(why) => why,
            })
        })?;
    match answer.status {
        200 => Ok(answer.body),
        429 => Err(Problem::Later(retry_after(&answer).unwrap_or(LATER_BUSY))),
        403 => Err(Problem::Later(LATER_REFUSED)),
        500..=599 => Err(Problem::Later(LATER_BUSY)),
        status => Err(Problem::Failed(format!(
            "PCGamingWiki answered HTTP {status}"
        ))),
    }
}

/// `Retry-After` in whole seconds, at most [`LATER_REFUSED`]; a date is not read.
fn retry_after(answer: &Answer) -> Option<u32> {
    let (_, value) =
        (answer.headers.iter()).find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))?;
    let seconds: u32 = value.trim().parse().ok()?;
    Some(seconds.clamp(1, LATER_REFUSED))
}

#[cfg(target_arch = "wasm32")]
#[allow(clippy::same_length_and_capacity, unsafe_code)]
mod plugin {
    wit_bindgen::generate!({
        world: "kidou:source/source@2.0.0",
        path: ["../../wit/deps/host", "../../wit/source"],
        generate_all,
    });

    use exports::kidou::source::lookup::Guest;
    use kidou::host::http::{self, HttpError};
    use kidou::source::types::{Fact, Found, Game, Language, Problem, Setting, Wanted};

    use crate::{Answer, Unsent};

    struct PcGamingWiki;

    /// Kidō's `http` import.
    struct Host;

    impl crate::Get for Host {
        fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<Answer, Unsent> {
            let headers: Vec<(String, String)> = (headers.iter())
                .map(|&(name, value)| (name.to_owned(), value.to_owned()))
                .collect();
            match http::get(url, &headers) {
                Ok(answer) => Ok(Answer {
                    status: answer.status,
                    headers: answer.headers,
                    body: answer.body,
                }),
                Err(HttpError::Denied) => Err(Unsent::Denied),
                Err(HttpError::TooLarge) => Err(Unsent::TooLarge),
                Err(HttpError::Unreachable(why)) => Err(Unsent::Unreachable(why)),
                Err(HttpError::Failed(why)) => Err(Unsent::Failed(why)),
            }
        }
    }

    impl Guest for PcGamingWiki {
        fn about(
            game: Game,
            wanted: Wanted,
            language: Language,
            _settings: Vec<Setting>,
        ) -> Result<Found, Problem> {
            let ask = crate::Ask {
                store: &game.store,
                store_id: &game.key,
                title: &game.title,
                facts: wanted.facts,
                language: match language {
                    Language::English => crate::Language::English,
                    Language::German => crate::Language::German,
                },
            };
            match crate::about(&mut Host, &ask) {
                Ok(rows) => Ok(Found {
                    art: Vec::new(),
                    facts: (rows.into_iter())
                        .map(|row| Fact {
                            label: row.label,
                            value: row.value,
                        })
                        .collect(),
                }),
                Err(crate::Problem::Later(seconds)) => Err(Problem::Later(seconds)),
                Err(crate::Problem::Failed(why)) => Err(Problem::Failed(why)),
            }
        }
    }

    export!(PcGamingWiki);
}

#[cfg(test)]
mod tests;
