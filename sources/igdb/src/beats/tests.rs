//! The asks against Twitch's and IGDB's answers as a fake gives them: Twitch's token taken for
//! each call, IGDB's game found by the store's id, its times read, and every refusal, wish to
//! wait and empty answer. IGDB itself is asked only on the device.

use std::collections::{HashMap, VecDeque};

use super::{Answer, Post, Problem, Unsent};
use crate::api::{EXTERNAL_GAMES, TIMES_TO_BEAT, TOKEN};
use crate::facts::{Language, Row};
use crate::{Ask, about};

/// Obvious fakes, 30 letters and digits as Twitch's are.
const CLIENT_ID: &str = "fakeclientid000000000000000001";
const SECRET: &str = "fakeclientsecret00000000000001";
const TOKEN_ONE: &str = "faketokenone";

struct Posted {
    url: String,
    headers: Vec<(String, String)>,
    body: String,
}

/// Gives each URL's answers in turn, and a 404 once they run out.
#[derive(Default)]
struct FakeHttp {
    answers: HashMap<&'static str, VecDeque<Result<Answer, Unsent>>>,
    posted: Vec<Posted>,
}

impl FakeHttp {
    fn answer(&mut self, url: &'static str, answers: Vec<Result<Answer, Unsent>>) {
        self.answers.entry(url).or_default().extend(answers);
    }

    fn tokens(&self) -> usize {
        self.posted.iter().filter(|post| post.url == TOKEN).count()
    }
}

impl Post for FakeHttp {
    fn post(&mut self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Answer, Unsent> {
        self.posted.push(Posted {
            url: url.to_owned(),
            headers: (headers.iter())
                .map(|&(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
            body: String::from_utf8(body.to_vec()).unwrap(),
        });
        let next = self.answers.get_mut(url).and_then(VecDeque::pop_front);
        next.unwrap_or_else(|| status(404))
    }
}

#[expect(clippy::unnecessary_wraps, reason = "an answer as the fake gives it")]
fn answer(status: u16, headers: &[(&str, &str)], body: &str) -> Result<Answer, Unsent> {
    Ok(Answer {
        status,
        headers: (headers.iter())
            .map(|&(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
        body: body.as_bytes().to_vec(),
    })
}

fn token(token: &str) -> Result<Answer, Unsent> {
    let body =
        format!(r#"{{"access_token":"{token}","expires_in":5587808,"token_type":"bearer"}}"#);
    answer(200, &[], &body)
}

fn ok(body: &str) -> Result<Answer, Unsent> {
    answer(200, &[], body)
}

fn status(status: u16) -> Result<Answer, Unsent> {
    answer(status, &[], "")
}

/// Portal 2, as IGDB lists it: game 72, with its three times in seconds.
fn portal(http: &mut FakeHttp) {
    http.answer(TOKEN, vec![token(TOKEN_ONE)]);
    http.answer(EXTERNAL_GAMES, vec![ok(r#"[{"id":9,"game":72}]"#)]);
    http.answer(
        TIMES_TO_BEAT,
        vec![ok(
            r#"[{"id":3,"hastily":28800,"normally":36000,"completely":72000}]"#,
        )],
    );
}

fn ask<'a>(store: &'a str, store_id: &'a str) -> Ask<'a> {
    Ask {
        store,
        store_id,
        facts: true,
        language: Language::English,
        client_id: CLIENT_ID,
        client_secret: SECRET,
    }
}

fn row(label: &str, value: &str) -> Row {
    Row {
        label: label.to_owned(),
        value: value.to_owned(),
    }
}

fn portal_rows() -> Vec<Row> {
    vec![row("Main story", "≈ 8 h")]
}

#[test]
fn a_token_then_the_game_by_its_store_id_then_its_times() {
    let mut http = FakeHttp::default();
    portal(&mut http);
    assert_eq!(about(&mut http, &ask("steam", "620")), Ok(portal_rows()));
    let [twitch, listed, times] = &http.posted[..] else {
        panic!("three posts");
    };
    assert_eq!(twitch.url, TOKEN);
    assert!((twitch.headers.iter()).any(
        |(name, value)| name == "content-type" && value == "application/x-www-form-urlencoded"
    ));
    assert_eq!(
        twitch.body,
        format!("client_id={CLIENT_ID}&client_secret={SECRET}&grant_type=client_credentials")
    );
    assert_eq!(listed.url, EXTERNAL_GAMES);
    assert_eq!(
        listed.body,
        "fields game; where uid = \"620\" & external_game_source = 1; limit 1;"
    );
    assert_eq!(times.url, TIMES_TO_BEAT);
    assert_eq!(
        times.body,
        "fields hastily,normally,completely; where game_id = 72; limit 1;"
    );
    for igdb in [listed, times] {
        let header = |name: &str| {
            (igdb.headers.iter())
                .find(|(known, _)| known == name)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(header("Client-ID").as_deref(), Some(CLIENT_ID));
        assert_eq!(header("Authorization"), Some(format!("Bearer {TOKEN_ONE}")));
        // The secret goes to Twitch alone.
        assert!(!igdb.body.contains(SECRET));
        assert!(
            igdb.headers
                .iter()
                .all(|(_, value)| !value.contains(SECRET))
        );
    }
}

#[test]
fn a_german_row_and_a_gog_game_under_igdb_s_source_5() {
    let mut http = FakeHttp::default();
    portal(&mut http);
    let german = Ask {
        language: Language::German,
        ..ask("gog", "1207658924")
    };
    assert_eq!(
        about(&mut http, &german),
        Ok(vec![row("Hauptstory", "≈ 8 Std.")])
    );
    assert!(http.posted[1].body.contains("external_game_source = 5;"));
}

#[test]
fn each_call_takes_one_token_for_both_of_igdb_s_asks() {
    let mut http = FakeHttp::default();
    portal(&mut http);
    portal(&mut http);
    about(&mut http, &ask("steam", "620")).unwrap();
    assert_eq!(http.tokens(), 1);
    about(&mut http, &ask("steam", "620")).unwrap();
    assert_eq!(http.tokens(), 2);
}

#[test]
fn nothing_is_asked_without_the_facts_both_settings_or_an_igdb_source() {
    let unasked = [
        Ask {
            facts: false,
            ..ask("steam", "620")
        },
        Ask {
            client_id: "",
            ..ask("steam", "620")
        },
        Ask {
            client_secret: "",
            ..ask("steam", "620")
        },
        ask("epic", "620"),
        ask("programs", "620"),
    ];
    for each in &unasked {
        let mut http = FakeHttp::default();
        portal(&mut http);
        assert_eq!(about(&mut http, each), Ok(Vec::new()));
        assert!(http.posted.is_empty());
    }
}

#[test]
fn a_setting_that_is_no_key_is_refused_unsent() {
    let mut http = FakeHttp::default();
    let wrong = Ask {
        client_secret: "not a secret",
        ..ask("steam", "620")
    };
    assert_eq!(
        about(&mut http, &wrong),
        Err(Problem::Refused("client_secret".to_owned()))
    );
    assert!(http.posted.is_empty());
}

/// A case: what it is, Twitch's, IGDB's `external_games` and its `game_time_to_beats`'s
/// answers, and what the call gives.
type Case = (
    &'static str,
    Vec<Result<Answer, Unsent>>,
    Vec<Result<Answer, Unsent>>,
    Vec<Result<Answer, Unsent>>,
    Result<Vec<Row>, Problem>,
);

fn each_case(cases: Vec<Case>) {
    for (case, twitch, listed, times, expected) in cases {
        let mut http = FakeHttp::default();
        http.answer(TOKEN, twitch);
        http.answer(EXTERNAL_GAMES, listed);
        http.answer(TIMES_TO_BEAT, times);
        assert_eq!(
            about(&mut http, &ask("gog", "1207658924")),
            expected,
            "{case}"
        );
    }
}

fn refused(key: &str) -> Result<Vec<Row>, Problem> {
    Err(Problem::Refused(key.to_owned()))
}

fn failed(why: &str) -> Result<Vec<Row>, Problem> {
    Err(Problem::Failed(why.to_owned()))
}

#[test]
fn refusals() {
    each_case(vec![
        (
            "Twitch knows no such client",
            vec![answer(
                400,
                &[],
                r#"{"status":400,"message":"invalid client"}"#,
            )],
            vec![],
            vec![],
            refused("client_id"),
        ),
        (
            "Twitch refuses the secret",
            vec![answer(
                403,
                &[],
                r#"{"status":403,"message":"invalid client secret"}"#,
            )],
            vec![],
            vec![],
            refused("client_secret"),
        ),
        (
            "Twitch refuses without a word",
            vec![status(401)],
            vec![],
            vec![],
            refused("client_secret"),
        ),
        (
            "IGDB refuses the token just given",
            vec![token(TOKEN_ONE)],
            vec![status(401)],
            vec![],
            refused("client_id"),
        ),
    ]);
}

#[test]
fn empty_answers() {
    each_case(vec![
        (
            "IGDB does not list the game",
            vec![token(TOKEN_ONE)],
            vec![ok("[]")],
            vec![],
            Ok(vec![]),
        ),
        (
            "IGDB knows no time to beat",
            vec![token(TOKEN_ONE)],
            vec![ok(r#"[{"id":9,"game":72}]"#)],
            vec![ok("[]")],
            Ok(vec![]),
        ),
        (
            "IGDB's times are all zero",
            vec![token(TOKEN_ONE)],
            vec![ok(r#"[{"id":9,"game":72}]"#)],
            vec![ok(r#"[{"id":3,"hastily":0}]"#)],
            Ok(vec![]),
        ),
        (
            "IGDB tells the main story alone",
            vec![token(TOKEN_ONE)],
            vec![ok(r#"[{"id":9,"game":72}]"#)],
            vec![ok(r#"[{"id":3,"hastily":5400}]"#)],
            Ok(vec![row("Main story", "≈ 2 h")]),
        ),
        (
            "IGDB tells no main story",
            vec![token(TOKEN_ONE)],
            vec![ok(r#"[{"id":9,"game":72}]"#)],
            vec![ok(r#"[{"id":3,"normally":9000,"completely":18000}]"#)],
            Ok(vec![row("With extras", "≈ 3 h")]),
        ),
    ]);
}

#[test]
fn wishes_to_wait_and_failures() {
    each_case(vec![
        (
            "IGDB asks to wait",
            vec![token(TOKEN_ONE)],
            vec![answer(429, &[("Retry-After", "4")], "")],
            vec![],
            Err(Problem::Later(4)),
        ),
        (
            "IGDB asks to wait until a date",
            vec![token(TOKEN_ONE)],
            vec![answer(
                503,
                &[
                    ("Date", "Sun, 22 Nov 2026 09:00:00 GMT"),
                    ("Retry-After", "Sun, 22 Nov 2026 10:00:00 GMT"),
                ],
                "",
            )],
            vec![],
            Err(Problem::Later(3600)),
        ),
        (
            "Twitch asks to wait until a leap day, a year on",
            vec![answer(
                429,
                &[
                    ("date", "Thu, 29 Feb 2024 23:59:59 GMT"),
                    ("retry-after", "Sat, 01 Mar 2025 00:00:00 GMT"),
                ],
                "",
            )],
            vec![],
            vec![],
            Err(Problem::Later(365 * 86_400 + 1)),
        ),
        (
            "Twitch asks to wait until a date, without its own",
            vec![answer(
                429,
                &[("Retry-After", "Sun, 22 Nov 2026 10:00:00 GMT")],
                "",
            )],
            vec![],
            vec![],
            Err(Problem::Later(10)),
        ),
        (
            "Twitch is down",
            vec![status(503)],
            vec![],
            vec![],
            Err(Problem::Later(10)),
        ),
        (
            "IGDB fails",
            vec![token(TOKEN_ONE)],
            vec![status(500)],
            vec![],
            failed("IGDB answered 500"),
        ),
        (
            "Twitch's answer is too long",
            vec![Err(Unsent::TooLarge)],
            vec![],
            vec![],
            failed("Twitch's answer is too long"),
        ),
        (
            "Twitch is not granted",
            vec![Err(Unsent::Denied)],
            vec![],
            vec![],
            failed("Twitch could not be asked: the address is not granted"),
        ),
        (
            "There is no network",
            vec![Err(Unsent::Unreachable("offline".to_owned()))],
            vec![],
            vec![],
            failed("Twitch could not be asked: no network: offline"),
        ),
        (
            "IGDB's answer does not read",
            vec![token(TOKEN_ONE)],
            vec![ok("<html>")],
            vec![],
            failed("IGDB's answer does not read"),
        ),
    ]);
}

#[test]
fn a_store_id_that_is_not_digits_is_never_sent() {
    let mut http = FakeHttp::default();
    portal(&mut http);
    for id in ["", "620\" | id > 0", "x620", "123456789012345678901"] {
        assert_eq!(
            about(&mut http, &ask("steam", id)),
            Err(Problem::Failed(
                "the store's id is not digits alone; IGDB is not asked".to_owned()
            )),
            "{id:?}: a check that cannot run tells no time"
        );
    }
    assert!(http.posted.is_empty());
}

#[test]
fn no_problem_holds_the_credentials_or_the_token() {
    let failures = [
        vec![
            (TOKEN, token(TOKEN_ONE)),
            (EXTERNAL_GAMES, answer(500, &[], TOKEN_ONE)),
        ],
        vec![(TOKEN, answer(400, &[], SECRET))],
        vec![(TOKEN, Err(Unsent::Failed("refused".to_owned())))],
        vec![(TOKEN, ok(&format!(r#"{{"access_token":"{SECRET} x"}}"#)))],
    ];
    for answers in failures {
        let mut http = FakeHttp::default();
        for (url, each) in answers {
            http.answer(url, vec![each]);
        }
        let problem = about(&mut http, &ask("steam", "620")).unwrap_err();
        let text = format!("{problem:?}");
        for secret in [CLIENT_ID, SECRET, TOKEN_ONE] {
            assert!(!text.contains(secret), "{text}");
        }
    }
}
