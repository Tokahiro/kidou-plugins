//! The lookup: SteamGridDB asked for a game's art by its store's id where its art endpoints take
//! that store's ids (Steam's), else by its own id for the game, found from the store's id
//! (`/games/gog/<id>`) or, when it does not know that, from the game's exact title.
//!
//! The key goes in the `Authorization` header of requests of [`api::BASE`] and nowhere else. No
//! text made here holds it: SteamGridDB's own texts and the network's errors have it cut out
//! before they reach a [`Problem`].

use std::fmt;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use crate::api::{self, ApiRequest, Kind};
use crate::json::{self, AnswerError};
use crate::pick::{Picked, pick};
use crate::title;

/// The key of the setting that holds the player's key.
pub const KEY: &str = "api_key";

/// The least time between the starts of two requests of the API within one call. Kidō paces the
/// calls themselves, at least a second apart.
pub const PACE: Duration = Duration::from_secs(1);

/// How many seconds SteamGridDB is left alone when it asks for that without saying how long.
pub const LATER_FIRST: u32 = 60;

/// The most seconds SteamGridDB is left alone at a time, whatever it asks.
pub const LATER_MAX: u32 = 3600;

/// The candidates kept of each kind: the best, and successors in case its picture is gone.
pub const KEPT: usize = 3;

/// The longest text of SteamGridDB's or the network's kept in a failure.
const TEXT_MAX: usize = 200;

/// The game asked about: its store's id, its id in that store, and its title.
#[derive(Debug, Clone, Copy)]
pub struct Game<'a> {
    pub store: &'a str,
    pub id: &'a str,
    pub title: &'a str,
}

/// Why there is no answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// There is no key, SteamGridDB refused it, or it is not of a key's shape.
    Refused,
    /// SteamGridDB asks to be left alone for this many seconds.
    Later(u32),
    /// Anything else, without the key.
    Failed(String),
}

/// An answer of the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// The first header named `name`, in any case.
    fn header(&self, name: &str) -> Option<&str> {
        (self.headers.iter())
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// `Retry-After`, in seconds: a number of them, or a date, counted from the answer's `Date`
    /// or else from now.
    fn retry_after(&self) -> Option<u32> {
        let value = self.header("Retry-After")?.trim();
        if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Some(value.parse().unwrap_or(u32::MAX));
        }
        let until = http_date(value)?;
        let now = (self.header("Date"))
            .and_then(http_date)
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_or(0, |since| since.as_secs())
            });
        Some(u32::try_from(until.saturating_sub(now)).unwrap_or(u32::MAX))
    }
}

/// An HTTP date in the form every server sends (RFC 9110's IMF-fixdate,
/// `Sun, 06 Nov 1994 08:49:37 GMT`), as seconds since 1970.
fn http_date(value: &str) -> Option<u64> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let [_, day, month, year, time, "GMT"] = *value.split_ascii_whitespace().collect::<Vec<_>>()
    else {
        return None;
    };
    let day: u64 = day.parse().ok().filter(|day| (1..=31).contains(day))?;
    let month = u64::try_from(MONTHS.iter().position(|name| *name == month)?).ok()?;
    let year: u64 = (year.parse().ok()).filter(|year| (1970..=9999).contains(year))?;
    let [hours, minutes, seconds] = *(time.split(':'))
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?
    else {
        return None;
    };
    // 60 is a leap second.
    if hours > 23 || minutes > 59 || seconds > 60 {
        return None;
    }
    // Days since 1970 by the civil calendar (Howard Hinnant's `days_from_civil`), with the year
    // starting in March so that a leap day ends it.
    let (year, month) = match month.checked_sub(2) {
        Some(month) => (year, month),
        None => (year.checked_sub(1)?, month.checked_add(10)?),
    };
    let era = year.checked_div(400)?;
    let of_era = year.checked_rem(400)?;
    let of_year = (month.checked_mul(153)?.checked_add(2)?.checked_div(5)?)
        .checked_add(day)?
        .checked_sub(1)?;
    let of_cycle = (of_era.checked_mul(365)?)
        .checked_add(of_era.checked_div(4)?)?
        .checked_sub(of_era.checked_div(100)?)?
        .checked_add(of_year)?;
    let days = (era.checked_mul(146_097)?)
        .checked_add(of_cycle)?
        .checked_sub(719_468)?;
    (days.checked_mul(86_400)?)
        .checked_add(hours.checked_mul(3_600)?)?
        .checked_add(minutes.checked_mul(60)?)?
        .checked_add(seconds)
}

/// Why the network gave no answer, as the host's `http` says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    Denied,
    TooLarge,
    Unreachable(String),
    Failed(String),
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied => f.write_str("Kidō denied the request"),
            Self::TooLarge => f.write_str("the answer is larger than Kidō reads"),
            Self::Unreachable(why) => write!(f, "no network: {why}"),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

/// HTTPS GETs.
pub trait Http {
    fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<Response, HttpError>;
}

/// The time requests are paced by.
pub trait Clock {
    fn now(&self) -> Instant;
    /// Blocks the calling thread for `duration`.
    fn sleep(&mut self, duration: Duration);
}

/// The system's monotonic clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

/// The art of `game` of the `kinds` asked for, through `http`, one request at a time and at most
/// one a second ([`PACE`]), the [`KEPT`] best of each kind in the order the kinds are asked for.
///
/// `key` is the setting [`KEY`] as the player typed it in. Empty or not of a key's shape, it is
/// [`Problem::Refused`] without a request; refused with a 401 or 403, it is too. A 404
/// for the game, or no picture of any kind, finds nothing. A 429 or 5xx is [`Problem::Later`].
/// Anything else that goes wrong is [`Problem::Failed`].
pub fn about(
    http: &mut dyn Http,
    clock: &mut dyn Clock,
    game: &Game<'_>,
    kinds: &[Kind],
    key: &str,
) -> Result<Vec<Picked>, Problem> {
    let key = key.trim();
    if !shaped(key) {
        return Err(Problem::Refused);
    }
    let mut lookup = Lookup {
        http,
        clock,
        last: None,
        bearer: format!("Bearer {key}"),
    };
    lookup.find(game, kinds).map_err(|problem| match problem {
        Problem::Failed(text) => Problem::Failed(scrub(&text, key)),
        problem => problem,
    })
}

/// Whether `key` has a key's shape: 20 to 64 ASCII letters and digits.
fn shaped(key: &str) -> bool {
    (20..=64).contains(&key.len()) && key.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// One call's lookup.
struct Lookup<'a> {
    http: &'a mut dyn Http,
    clock: &'a mut dyn Clock,
    /// When the last request started.
    last: Option<Instant>,
    bearer: String,
}

impl Lookup<'_> {
    fn find(&mut self, game: &Game<'_>, kinds: &[Kind]) -> Result<Vec<Picked>, Problem> {
        let kinds = once_each(kinds);
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        let platform = (api::PLATFORMS.iter())
            .find(|(store, _)| *store == game.store)
            .map(|&(_, platform)| platform);
        let (platform, id) = match platform {
            Some(platform) if api::ART_PLATFORMS.contains(&platform) => {
                (platform, game.id.to_owned())
            }
            platform => {
                let by_id = match platform {
                    Some(platform) => self.by_id(platform, game.id)?,
                    None => None,
                };
                let found = match by_id {
                    Some(found) => Some(found),
                    None => self.by_title(game.title)?,
                };
                let Some(found) = found else {
                    return Ok(Vec::new());
                };
                (api::OWN, found.to_string())
            }
        };
        let mut picked = Vec::new();
        for (at, &kind) in kinds.iter().enumerate() {
            let request = api::art(kind, platform, &id);
            match self.send(&request)? {
                Reply::Answer(body) => {
                    let images = json::images(&body).map_err(|error| unread(&request, error))?;
                    picked.extend(pick(kind, &images).into_iter().take(KEPT));
                }
                // The platform's id is not known: no other kind will be either.
                Reply::NotFound if at == 0 => return Ok(Vec::new()),
                Reply::NotFound => {}
                Reply::Other(status, body) => return Err(failed(&request, status, &body)),
            }
        }
        Ok(picked)
    }

    /// SteamGridDB's own id of the game `id` of `platform`; `None` when it does not know it by
    /// that id, which leaves its title to search for.
    fn by_id(&mut self, platform: &str, id: &str) -> Result<Option<u64>, Problem> {
        let request = api::game(platform, id);
        match self.send(&request)? {
            Reply::Answer(body) => match json::game(&body) {
                Ok(game) => Ok(game.map(|game| game.id)),
                Err(AnswerError::Unsuccessful(_)) => Ok(None),
                Err(error) => Err(unread(&request, error)),
            },
            // It does not know the id, or not the platform (400, 422).
            Reply::NotFound | Reply::Other(400..=499, _) => Ok(None),
            Reply::Other(status, body) => Err(failed(&request, status, &body)),
        }
    }

    /// SteamGridDB's own id of the game of exactly this title (the rule of [`title::choose`]).
    fn by_title(&mut self, title: &str) -> Result<Option<u64>, Problem> {
        if title::normalized(title).is_empty() {
            return Ok(None);
        }
        let request = api::search(title);
        match self.send(&request)? {
            Reply::Answer(body) => {
                let found = json::games(&body).map_err(|error| unread(&request, error))?;
                Ok(title::choose(title, &found))
            }
            Reply::NotFound => Ok(None),
            Reply::Other(status, body) => Err(failed(&request, status, &body)),
        }
    }

    /// Sends `request` once its turn has come, and sorts the answer. A refused key, a request to
    /// wait and a failure on the way end the lookup.
    fn send(&mut self, request: &ApiRequest) -> Result<Reply, Problem> {
        self.pace();
        let url = request.url();
        let headers = [
            ("Authorization", self.bearer.as_str()),
            ("Accept", "application/json"),
        ];
        let response = (self.http.get(&url, &headers)).map_err(|error| lost(request, &error))?;
        let status = response.status;
        if status == 429 || (500..=599).contains(&status) {
            let wait = response.retry_after().unwrap_or(LATER_FIRST);
            return Err(Problem::Later(wait.clamp(1, LATER_MAX)));
        }
        if status == 401 || status == 403 {
            return Err(Problem::Refused);
        }
        Ok(match status {
            200..=299 => Reply::Answer(response.body),
            404 => Reply::NotFound,
            _ => Reply::Other(status, response.body),
        })
    }

    /// Waits until [`PACE`] has passed since the last request started.
    fn pace(&mut self) {
        let now = self.clock.now();
        let wait = (self.last.and_then(|last| last.checked_add(PACE)))
            .and_then(|next| next.checked_duration_since(now))
            .filter(|wait| !wait.is_zero());
        if let Some(wait) = wait {
            self.clock.sleep(wait);
        }
        self.last = Some(self.clock.now());
    }
}

/// An answer that neither refuses the key nor asks to wait.
enum Reply {
    /// 2xx, with its body.
    Answer(Vec<u8>),
    /// 404.
    NotFound,
    /// Any other status, with its body.
    Other(u16, Vec<u8>),
}

/// The kinds asked for, each once, in the order first asked.
fn once_each(kinds: &[Kind]) -> Vec<Kind> {
    let mut once = Vec::with_capacity(kinds.len());
    for &kind in kinds {
        if !once.contains(&kind) {
            once.push(kind);
        }
    }
    once
}

/// The first part of a request's path, which names what was asked without a game's title.
fn endpoint(request: &ApiRequest) -> &str {
    (request.path.split('/'))
        .find(|part| !part.is_empty())
        .unwrap_or_default()
}

fn lost(request: &ApiRequest, error: &HttpError) -> Problem {
    Problem::Failed(format!(
        "SteamGridDB's {} could not be asked: {error}",
        endpoint(request)
    ))
}

fn unread(request: &ApiRequest, error: AnswerError) -> Problem {
    Problem::Failed(match error {
        AnswerError::Malformed(why) => format!(
            "SteamGridDB's answer to {} is not readable: {why}",
            endpoint(request)
        ),
        AnswerError::Unsuccessful(errors) => format!(
            "SteamGridDB's {} did not succeed: {}",
            endpoint(request),
            errors.join("; ")
        ),
    })
}

fn failed(request: &ApiRequest, status: u16, body: &[u8]) -> Problem {
    let errors = json::errors(body);
    Problem::Failed(if errors.is_empty() {
        format!("SteamGridDB's {} answered {status}", endpoint(request))
    } else {
        format!(
            "SteamGridDB's {} answered {status}: {}",
            endpoint(request),
            errors.join("; ")
        )
    })
}

/// `text` without `key`, and cut to [`TEXT_MAX`] characters.
fn scrub(text: &str, key: &str) -> String {
    let clean = text.replace(key, "\u{2026}");
    if clean.chars().count() <= TEXT_MAX {
        clean
    } else {
        let mut cut = clean.chars().take(TEXT_MAX).collect::<String>();
        cut.push('\u{2026}');
        cut
    }
}

#[cfg(test)]
mod tests;
