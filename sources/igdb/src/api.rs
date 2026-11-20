//! Twitch's token and IGDB's two endpoints the plugin asks, with the bodies it sends and the
//! answers it reads (<https://api-docs.igdb.com/#getting-started>,
//! <https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#client-credentials-grant-flow>).
//! The bodies are made of the store's id, which is digits only, and IGDB's own numbers: no text
//! of a game's reaches a query.

use serde::Deserialize;

/// Twitch's token endpoint, which the client credentials are posted to as a form.
pub const TOKEN: &str = "https://id.twitch.tv/oauth2/token";

/// IGDB's list of the stores' ids for its games.
pub const EXTERNAL_GAMES: &str = "https://api.igdb.com/v4/external_games";

/// IGDB's times to beat, by its own game id.
pub const TIMES_TO_BEAT: &str = "https://api.igdb.com/v4/game_time_to_beats";

/// The settings' keys, as the manifest names them.
pub const CLIENT_ID: &str = "client_id";
pub const CLIENT_SECRET: &str = "client_secret";

/// The player's client id and secret from Twitch's developer console. No `Debug`: it holds them.
pub struct Credentials {
    pub(crate) id: String,
    pub(crate) secret: String,
}

impl Credentials {
    /// The credentials in the settings' texts, each without the white space around it; none
    /// while either is empty. A text that is no key (20 to 64 ASCII letters and digits, as
    /// Kidō's own settings take) is refused under its setting's key, unsent.
    pub fn of(id: &str, secret: &str) -> Result<Option<Self>, String> {
        let (id, secret) = (id.trim(), secret.trim());
        if id.is_empty() || secret.is_empty() {
            return Ok(None);
        }
        let key = |text: &str| {
            (20..=64).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric())
        };
        if !key(id) {
            return Err(CLIENT_ID.to_owned());
        }
        if !key(secret) {
            return Err(CLIENT_SECRET.to_owned());
        }
        Ok(Some(Self {
            id: id.to_owned(),
            secret: secret.to_owned(),
        }))
    }
}

/// The form that asks Twitch for an app access token with `credentials`.
pub fn token_form(credentials: &Credentials) -> String {
    format!(
        "client_id={}&client_secret={}&grant_type=client_credentials",
        credentials.id, credentials.secret
    )
}

/// IGDB's `external_game_source` for the store of id `store`, under which it lists the store's
/// own ids: Steam's app id and GOG's product id; none for another store.
pub fn source(store: &str) -> Option<u16> {
    match store {
        "steam" => Some(1),
        "gog" => Some(5),
        _ => None,
    }
}

/// The query for the IGDB game that the store of `source` lists as `uid`; `None` for a store id
/// that is not digits alone, which is never sent.
pub fn external_game(uid: &str, source: u16) -> Option<String> {
    let digits = !uid.is_empty() && uid.len() <= 20 && uid.bytes().all(|b| b.is_ascii_digit());
    digits.then(|| {
        format!("fields game; where uid = \"{uid}\" & external_game_source = {source}; limit 1;")
    })
}

/// The query for the times to beat of IGDB's game `game`.
pub fn times_to_beat(game: u64) -> String {
    format!("fields hastily,normally,completely; where game_id = {game}; limit 1;")
}

/// How long a game takes to beat, in whole minutes, as IGDB's players report it on average;
/// each `None` where IGDB tells none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeToBeat {
    /// The main story, to the credits without the extras (IGDB's `hastily`).
    pub main: Option<u32>,
    /// The main story and some of the extras (`normally`).
    pub extras: Option<u32>,
    /// Everything, to 100 % (`completely`).
    pub complete: Option<u32>,
}

impl TimeToBeat {
    /// The times IGDB tells in seconds, each rounded to the nearest minute; a time of zero, or
    /// one too long for the minutes to hold, is none.
    pub fn from_seconds(main: Option<u64>, extras: Option<u64>, complete: Option<u64>) -> Self {
        let minutes = |seconds: Option<u64>| {
            let minutes = seconds?.saturating_add(30) / 60;
            u32::try_from(minutes).ok().filter(|&minutes| minutes > 0)
        };
        Self {
            main: minutes(main),
            extras: minutes(extras),
            complete: minutes(complete),
        }
    }

    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

#[derive(Deserialize)]
struct TokenJson {
    access_token: String,
}

#[derive(Deserialize)]
struct RefusalJson {
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct ExternalGameJson {
    #[serde(default)]
    game: Option<u64>,
}

#[derive(Deserialize)]
struct TimeToBeatJson {
    #[serde(default)]
    hastily: Option<u64>,
    #[serde(default)]
    normally: Option<u64>,
    #[serde(default)]
    completely: Option<u64>,
}

/// Twitch's token. A token that could not stand in a header as it is, or none, is no answer.
pub fn token(body: &[u8]) -> Result<String, String> {
    let answer: TokenJson =
        serde_json::from_slice(body).map_err(|_| "Twitch's answer does not read".to_owned())?;
    let token = answer.access_token;
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("Twitch's answer holds no token".to_owned());
    }
    Ok(token)
}

/// The setting Twitch refused in its answer `body` to the token form: the client id where
/// Twitch says it knows no such client (`invalid client`), else the secret.
pub fn refused(body: &[u8]) -> &'static str {
    let message = serde_json::from_slice::<RefusalJson>(body).map(|answer| answer.message);
    match message {
        Ok(message) if message.trim().eq_ignore_ascii_case("invalid client") => CLIENT_ID,
        _ => CLIENT_SECRET,
    }
}

/// IGDB's game id in an answer of `external_games`; `None` when it lists none.
pub fn game(body: &[u8]) -> Result<Option<u64>, String> {
    let listed: Vec<ExternalGameJson> =
        serde_json::from_slice(body).map_err(|_| "IGDB's answer does not read".to_owned())?;
    Ok(listed.into_iter().find_map(|each| each.game))
}

/// The times to beat in an answer of `game_time_to_beats`; `None` when it tells none.
pub fn time_to_beat(body: &[u8]) -> Result<Option<TimeToBeat>, String> {
    let told: Vec<TimeToBeatJson> =
        serde_json::from_slice(body).map_err(|_| "IGDB's answer does not read".to_owned())?;
    Ok(told
        .into_iter()
        .map(|each| TimeToBeat::from_seconds(each.hastily, each.normally, each.completely))
        .find(|beat| !beat.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::{Credentials, TimeToBeat, refused, source, token};

    #[test]
    fn igdb_s_seconds_become_whole_minutes_and_zero_is_none() {
        let beat = TimeToBeat::from_seconds(Some(21_629), Some(29), None);
        assert_eq!(
            beat,
            TimeToBeat {
                main: Some(360),
                extras: None,
                complete: None,
            }
        );
        assert!(!beat.is_empty());
        let rounded = TimeToBeat::from_seconds(Some(30), Some(0), Some(89));
        assert_eq!((rounded.main, rounded.complete), (Some(1), Some(1)));
        assert!(TimeToBeat::from_seconds(Some(0), None, Some(u64::MAX)).is_empty());
    }

    #[test]
    fn credentials_are_both_keys_or_none() {
        let id = "fakeclientid000000000000000001";
        let secret = "fakeclientsecret00000000000001";
        assert!(matches!(Credentials::of("", secret), Ok(None)));
        assert!(matches!(Credentials::of(id, "  "), Ok(None)));
        let trimmed = Credentials::of(&format!(" {id}\n"), secret);
        assert!(matches!(trimmed, Ok(Some(ref both)) if both.id == id && both.secret == secret));
        for (id, secret, key) in [
            ("short", secret, "client_id"),
            (id, "fakeclientsecret0000000000&x=1", "client_secret"),
            (id, "fake client secret 0000000001", "client_secret"),
        ] {
            assert_eq!(Credentials::of(id, secret).err().as_deref(), Some(key));
        }
    }

    #[test]
    fn steam_and_gog_alone_have_an_igdb_source() {
        assert_eq!(source("steam"), Some(1));
        assert_eq!(source("gog"), Some(5));
        assert_eq!(source("epic"), None);
        assert_eq!(source("Steam"), None);
    }

    #[test]
    fn a_token_is_text_that_stands_in_a_header() {
        let answer = |token: &str| {
            format!(r#"{{"access_token":"{token}","expires_in":5587808,"token_type":"bearer"}}"#)
        };
        assert_eq!(token(answer("abc123").as_bytes()).as_deref(), Ok("abc123"));
        for wrong in ["", "two words", "line\\nbreak"] {
            assert!(token(answer(wrong).as_bytes()).is_err(), "{wrong:?}");
        }
        assert!(token(b"<html>").is_err());
    }

    #[test]
    fn twitch_names_the_client_id_or_the_secret_it_refuses() {
        assert_eq!(
            refused(br#"{"status":400,"message":"invalid client"}"#),
            "client_id"
        );
        assert_eq!(
            refused(br#"{"status":403,"message":"invalid client secret"}"#),
            "client_secret"
        );
        assert_eq!(refused(b""), "client_secret");
    }
}
