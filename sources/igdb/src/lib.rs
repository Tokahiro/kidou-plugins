//! IGDB as a source plugin with the manifest `plugin.toml`: the time to beat of a Steam or GOG
//! game, as IGDB's players report it, on the game's page (Kidō's ADR-0024).
//!
//! It asks with the player's own Twitch credentials, the settings `client_id` and
//! `client_secret`: the secret goes only to Twitch, for a token, and the client id and the token
//! only to IGDB; none of them into a problem's text. A game is asked for by its store's id alone.
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
mod beats;
mod facts;

use beats::{Post, Problem};
use facts::{Language, Row};

/// What Kidō asks of a game, with the settings' texts.
pub struct Ask<'a> {
    pub store: &'a str,
    pub key: &'a str,
    /// Whether the game's page shows, which needs the facts.
    pub facts: bool,
    pub language: Language,
    pub client_id: &'a str,
    pub client_secret: &'a str,
}

/// The rows of facts IGDB tells for `ask`. Nothing, and no request, while the facts are not
/// wanted, a setting is empty or the game's store has no source at IGDB.
pub fn about(http: &mut dyn Post, ask: &Ask<'_>) -> Result<Vec<Row>, Problem> {
    if !ask.facts {
        return Ok(Vec::new());
    }
    let credentials = api::Credentials::of(ask.client_id, ask.client_secret);
    let Some(credentials) = credentials.map_err(Problem::Refused)? else {
        return Ok(Vec::new());
    };
    let Some(source) = api::source(ask.store) else {
        return Ok(Vec::new());
    };
    let beat = beats::time_to_beat(http, ask.key, source, &credentials)?;
    Ok(beat.map_or_else(Vec::new, |beat| facts::rows(beat, ask.language)))
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
    use kidou::source::types::{Fact, Found, Game, Language, Problem, Setting, Value, Wanted};

    use crate::api::{CLIENT_ID, CLIENT_SECRET};
    use crate::beats::{self, Answer, Unsent};

    struct Igdb;

    /// Kidō's `http` import.
    struct Host;

    impl crate::Post for Host {
        fn post(
            &mut self,
            url: &str,
            headers: &[(&str, &str)],
            body: &[u8],
        ) -> Result<Answer, Unsent> {
            let headers: Vec<(String, String)> = (headers.iter())
                .map(|&(name, value)| (name.to_owned(), value.to_owned()))
                .collect();
            match http::post(url, &headers, body) {
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

    fn text<'a>(settings: &'a [Setting], key: &str) -> &'a str {
        match settings.iter().find(|setting| setting.key == key) {
            Some(Setting {
                value: Value::Text(text),
                ..
            }) => text,
            _ => "",
        }
    }

    impl Guest for Igdb {
        fn about(
            game: Game,
            wanted: Wanted,
            language: Language,
            settings: Vec<Setting>,
        ) -> Result<Found, Problem> {
            let ask = crate::Ask {
                store: &game.store,
                key: &game.key,
                facts: wanted.facts,
                language: match language {
                    Language::English => crate::Language::English,
                    Language::German => crate::Language::German,
                },
                client_id: text(&settings, CLIENT_ID),
                client_secret: text(&settings, CLIENT_SECRET),
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
                Err(beats::Problem::Refused(key)) => Err(Problem::Refused(key)),
                Err(beats::Problem::Later(seconds)) => Err(Problem::Later(seconds)),
                Err(beats::Problem::Failed(why)) => Err(Problem::Failed(why)),
            }
        }
    }

    export!(Igdb);
}
