//! The asks of a game's time to beat: a token from Twitch, IGDB's game found by the store's id,
//! its times to beat. Kidō makes a new instance for each call, so each call takes a token of its
//! own, for both of IGDB's asks.

use crate::api::{self, CLIENT_ID, Credentials, TimeToBeat};

/// How long IGDB or Twitch is left alone, in seconds, when it asks for that without saying how
/// long.
const LATER: u32 = 10;

/// An answer to a POST: its status, its headers and its body.
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Answer {
    /// What an answer whose status the ask does not read means: a wish to be left alone for
    /// its `Retry-After` in seconds, else [`LATER`], for a 429 or a 503 (the two statuses
    /// RFC 6585 and RFC 9110 give `Retry-After` to); else a failure that names `host`.
    fn later_or_failed(&self, host: &str) -> Problem {
        if !matches!(self.status, 429 | 503) {
            return Problem::Failed(format!("{host} answered {}", self.status));
        }
        let after = (self.headers.iter())
            .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
            .map(|(_, value)| value.trim())
            .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()));
        Problem::Later(after.map_or(LATER, |seconds| seconds.parse().unwrap_or(u32::MAX)))
    }
}

/// Why a POST got no answer, as Kidō's `http-error` tells it.
#[derive(Debug)]
pub enum Unsent {
    Denied,
    TooLarge,
    Unreachable(String),
    Failed(String),
}

impl Unsent {
    fn failed(self, host: &str) -> Problem {
        Problem::Failed(match self {
            Self::TooLarge => format!("{host}'s answer is too long"),
            Self::Denied => format!("{host} could not be asked: the address is not granted"),
            Self::Unreachable(why) => format!("{host} could not be asked: no network: {why}"),
            Self::Failed(why) => format!("{host} could not be asked: {why}"),
        })
    }
}

/// One POST at a time, to a host of the plugin's grants.
pub trait Post {
    fn post(&mut self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Answer, Unsent>;
}

/// Why there is no time to beat, as `kidou:source`'s `problem` tells it: the key of the setting
/// refused, the seconds to leave the service alone, or a failure for Kidō's log. No text holds
/// the credentials or the token.
#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    Refused(String),
    Later(u32),
    Failed(String),
}

/// The times to beat IGDB tells of the game the store of IGDB's `source` lists as `key`; none
/// where IGDB lists no such game or tells no time.
///
/// Twitch's 400, 401 or 403 to the credentials, or IGDB's 401 to the token Twitch just gave for
/// them, is [`Problem::Refused`]. A store id that is not digits alone is never sent.
pub fn time_to_beat(
    http: &mut dyn Post,
    key: &str,
    source: u16,
    credentials: &Credentials,
) -> Result<Option<TimeToBeat>, Problem> {
    let Some(query) = api::external_game(key, source) else {
        return Err(Problem::Failed(
            "the store's id is not digits alone; IGDB is not asked".into(),
        ));
    };
    let bearer = format!("Bearer {}", token(http, credentials)?);
    let listed = igdb(http, api::EXTERNAL_GAMES, &query, credentials, &bearer)?;
    let Some(game) = api::game(&listed).map_err(Problem::Failed)? else {
        return Ok(None);
    };
    let times = api::times_to_beat(game);
    let told = igdb(http, api::TIMES_TO_BEAT, &times, credentials, &bearer)?;
    api::time_to_beat(&told).map_err(Problem::Failed)
}

/// A token from Twitch for `credentials`.
fn token(http: &mut dyn Post, credentials: &Credentials) -> Result<String, Problem> {
    let headers = [
        ("Accept", "application/json"),
        ("content-type", "application/x-www-form-urlencoded"),
    ];
    let form = api::token_form(credentials);
    let answer = (http.post(api::TOKEN, &headers, form.as_bytes()))
        .map_err(|unsent| unsent.failed("Twitch"))?;
    match answer.status {
        200 => api::token(&answer.body).map_err(Problem::Failed),
        400 | 401 | 403 => Err(Problem::Refused(api::refused(&answer.body).to_owned())),
        _ => Err(answer.later_or_failed("Twitch")),
    }
}

/// Posts `query` to IGDB's `url` with the token in `bearer`, and gives the answer's body.
fn igdb(
    http: &mut dyn Post,
    url: &str,
    query: &str,
    credentials: &Credentials,
    bearer: &str,
) -> Result<Vec<u8>, Problem> {
    let headers = [
        ("Client-ID", credentials.id.as_str()),
        ("Authorization", bearer),
        ("Accept", "application/json"),
        ("content-type", "text/plain"),
    ];
    let answer =
        (http.post(url, &headers, query.as_bytes())).map_err(|unsent| unsent.failed("IGDB"))?;
    match answer.status {
        200..=299 => Ok(answer.body),
        401 => Err(Problem::Refused(CLIENT_ID.to_owned())),
        _ => Err(answer.later_or_failed("IGDB")),
    }
}

#[cfg(test)]
mod tests;
