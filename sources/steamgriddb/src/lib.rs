//! SteamGridDB as a source plugin, with the manifest `plugin.toml`: art for any game, asked with
//! the player's own key (the setting `api_key`), which goes to SteamGridDB's API and nowhere
//! else: never to an image host, never into a problem's text. It gives no facts.
//!
//! Its answers come from the network, so reading them must not panic (the release build aborts
//! on a panic): no indexing, unwrapping or unchecked arithmetic outside the tests.

#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::arithmetic_side_effects
)]
#![cfg_attr(
    test,
    allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::arithmetic_side_effects
    )
)]

pub mod api;
pub mod json;
pub mod lookup;
pub mod pick;
pub mod title;

#[cfg(target_arch = "wasm32")]
#[allow(clippy::same_length_and_capacity)]
mod plugin {
    wit_bindgen::generate!({
        world: "kidou:source/source@2.0.0",
        path: ["../../wit/deps/host", "../../wit/source"],
        generate_all,
    });

    use exports::kidou::source::lookup::Guest;
    use kidou::host::http;
    use kidou::source::types::{
        Art, ArtKind, Found, Game, Language, Problem, Setting, Value, Wanted,
    };

    use crate::api::Kind;
    use crate::lookup::{self, HttpError, Response, SystemClock};

    struct SteamGridDb;

    impl From<ArtKind> for Kind {
        fn from(kind: ArtKind) -> Self {
            match kind {
                ArtKind::Square => Self::Square,
                ArtKind::Cover => Self::Cover,
                ArtKind::Hero => Self::Hero,
                ArtKind::Logo => Self::Logo,
            }
        }
    }

    impl From<Kind> for ArtKind {
        fn from(kind: Kind) -> Self {
            match kind {
                Kind::Square => Self::Square,
                Kind::Cover => Self::Cover,
                Kind::Hero => Self::Hero,
                Kind::Logo => Self::Logo,
            }
        }
    }

    /// The host's `http`.
    struct Host;

    impl lookup::Http for Host {
        fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<Response, HttpError> {
            let headers = (headers.iter())
                .map(|&(name, value)| (name.to_owned(), value.to_owned()))
                .collect::<Vec<_>>();
            http::get(url, &headers)
                .map(|response| Response {
                    status: response.status,
                    headers: response.headers,
                    body: response.body,
                })
                .map_err(|error| match error {
                    http::HttpError::Denied => HttpError::Denied,
                    http::HttpError::TooLarge => HttpError::TooLarge,
                    http::HttpError::Unreachable(why) => HttpError::Unreachable(why),
                    http::HttpError::Failed(why) => HttpError::Failed(why),
                })
        }
    }

    impl Guest for SteamGridDb {
        fn about(
            game: Game,
            wanted: Wanted,
            _language: Language,
            settings: Vec<Setting>,
        ) -> Result<Found, Problem> {
            let key = (settings.iter())
                .find(|setting| setting.key == lookup::KEY)
                .and_then(|setting| match &setting.value {
                    Value::Text(text) => Some(text.as_str()),
                    Value::Switch(_) | Value::Choice(_) => None,
                })
                .unwrap_or_default();
            let kinds = (wanted.art.into_iter()).map(Kind::from).collect::<Vec<_>>();
            let asked = lookup::Game {
                store: &game.store,
                id: &game.key,
                title: &game.title,
            };
            let picked = lookup::about(&mut Host, &mut SystemClock, &asked, &kinds, key).map_err(
                |problem| match problem {
                    lookup::Problem::Refused => Problem::Refused(lookup::KEY.to_owned()),
                    lookup::Problem::Later(seconds) => Problem::Later(seconds),
                    lookup::Problem::Failed(text) => Problem::Failed(text),
                },
            )?;
            let art = (picked.into_iter())
                .map(|picked| Art {
                    kind: picked.kind.into(),
                    url: picked.url,
                    titled: picked.titled,
                })
                .collect();
            Ok(Found {
                art,
                facts: Vec::new(),
            })
        }
    }

    export!(SteamGridDb);
}
