use std::collections::HashMap;

use super::*;

/// Every fixture, by its file name.
const FIXTURES: &[(&str, &[u8])] = &[
    (
        "empty.json",
        include_bytes!("../../tests/fixtures/empty.json"),
    ),
    (
        "errors.json",
        include_bytes!("../../tests/fixtures/errors.json"),
    ),
    (
        "game-gog.json",
        include_bytes!("../../tests/fixtures/game-gog.json"),
    ),
    (
        "grids-cover.json",
        include_bytes!("../../tests/fixtures/grids-cover.json"),
    ),
    (
        "grids-square.json",
        include_bytes!("../../tests/fixtures/grids-square.json"),
    ),
    (
        "heroes.json",
        include_bytes!("../../tests/fixtures/heroes.json"),
    ),
    (
        "not-found.json",
        include_bytes!("../../tests/fixtures/not-found.json"),
    ),
    (
        "search.json",
        include_bytes!("../../tests/fixtures/search.json"),
    ),
    (
        "too-many.json",
        include_bytes!("../../tests/fixtures/too-many.json"),
    ),
    (
        "unauthorized.json",
        include_bytes!("../../tests/fixtures/unauthorized.json"),
    ),
];

/// The API's base, as the tests spell every address out in full.
const API: &str = "https://www.steamgriddb.com/api/v2";

/// The query of every art request.
const SQUARES: &str = "?dimensions=512x512,1024x1024&types=static&nsfw=false&humor=false";
const COVERS: &str = "?dimensions=600x900,660x930&types=static&nsfw=false&humor=false";
const HEROES: &str = "?dimensions=1920x620&types=static&nsfw=false&humor=false";

const ALL: &[Kind] = &[Kind::Square, Kind::Cover, Kind::Hero];

fn fixture(name: &str) -> &'static [u8] {
    FIXTURES
        .iter()
        .find(|(known, _)| *known == name)
        .map_or_else(|| panic!("no fixture {name}"), |(_, bytes)| *bytes)
}

/// An answer with `status`, a JSON body and perhaps a `Retry-After`.
fn answer(status: u16, body: &[u8], retry_after: Option<u32>) -> Response {
    let mut headers = vec![(
        "Content-Type".to_owned(),
        "application/json; charset=utf-8".to_owned(),
    )];
    headers.extend(retry_after.map(|seconds| ("Retry-After".to_owned(), seconds.to_string())));
    Response {
        status,
        headers,
        body: body.to_vec(),
    }
}

fn ok(name: &str) -> Response {
    answer(200, fixture(name), None)
}

/// A network that answers each address as told, and a 404 to any other.
#[derive(Default)]
struct FakeHttp {
    answers: HashMap<String, Result<Response, HttpError>>,
    /// Each request's address and headers, in order.
    asked: Vec<(String, Vec<(String, String)>)>,
}

impl Http for FakeHttp {
    fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<Response, HttpError> {
        let headers = (headers.iter())
            .map(|&(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        self.asked.push((url.to_owned(), headers));
        (self.answers.get(url).cloned()).unwrap_or_else(|| Ok(answer(404, b"", None)))
    }
}

/// A clock that stands still but for what it is asked to sleep.
struct TestClock {
    now: Instant,
    slept: Vec<Duration>,
}

impl Clock for TestClock {
    fn now(&self) -> Instant {
        self.now
    }

    fn sleep(&mut self, duration: Duration) {
        self.now += duration;
        self.slept.push(duration);
    }
}

/// A lookup's network, clock and key.
struct Rig {
    http: FakeHttp,
    clock: TestClock,
    key: String,
}

impl Rig {
    fn new() -> Self {
        Self {
            http: FakeHttp::default(),
            clock: TestClock {
                now: Instant::now(),
                slept: Vec::new(),
            },
            key: "fake".repeat(8),
        }
    }

    fn answer(&mut self, url: &str, answer: Response) {
        self.http.answers.insert(url.to_owned(), Ok(answer));
    }

    fn fail(&mut self, url: &str, error: HttpError) {
        self.http.answers.insert(url.to_owned(), Err(error));
    }

    fn look_up(
        &mut self,
        store: &str,
        id: &str,
        title: &str,
        kinds: &[Kind],
    ) -> Result<Vec<Picked>, Problem> {
        let game = Game { store, id, title };
        about(&mut self.http, &mut self.clock, &game, kinds, &self.key)
    }

    fn steam(&mut self, id: &str, kinds: &[Kind]) -> Result<Vec<Picked>, Problem> {
        self.look_up("steam", id, "A Steam game", kinds)
    }

    fn gog(&mut self, id: &str, title: &str, kinds: &[Kind]) -> Result<Vec<Picked>, Problem> {
        self.look_up("gog", id, title, kinds)
    }

    /// The addresses asked for so far, in order.
    fn urls(&self) -> Vec<String> {
        self.http.asked.iter().map(|(url, _)| url.clone()).collect()
    }
}

/// The file names of the pictures found.
fn names(found: &Result<Vec<Picked>, Problem>) -> Vec<String> {
    let Ok(found) = found else {
        panic!("not found: {found:?}");
    };
    (found.iter())
        .map(|picked| picked.url.rsplit('/').next().unwrap().to_owned())
        .collect()
}

/// The name a fixture gives picture `id`.
fn named(id: u64, extension: &str) -> String {
    format!("{id:032x}.{extension}")
}

fn failure(found: Result<Vec<Picked>, Problem>) -> String {
    let Err(Problem::Failed(text)) = found else {
        panic!("did not fail: {found:?}");
    };
    text
}

fn square(platform: &str, id: &str) -> String {
    format!("{API}/grids/{platform}/{id}{SQUARES}")
}

fn cover(platform: &str, id: &str) -> String {
    format!("{API}/grids/{platform}/{id}{COVERS}")
}

fn hero(platform: &str, id: &str) -> String {
    format!("{API}/heroes/{platform}/{id}{HEROES}")
}

/// A Steam app whose art SteamGridDB has of every kind.
fn steam_rig() -> Rig {
    let mut rig = Rig::new();
    rig.answer(&square("steam", "620"), ok("grids-square.json"));
    rig.answer(&cover("steam", "620"), ok("grids-cover.json"));
    rig.answer(&hero("steam", "620"), ok("heroes.json"));
    rig
}

#[test]
fn each_kind_is_asked_for_once_in_the_order_first_asked() {
    assert_eq!(
        once_each(&[
            Kind::Hero,
            Kind::Square,
            Kind::Hero,
            Kind::Cover,
            Kind::Square
        ]),
        [Kind::Hero, Kind::Square, Kind::Cover]
    );
    assert_eq!(once_each(&[]), []);
}

#[test]
fn the_key_is_cut_out_of_a_text_and_a_text_is_cut_short() {
    let key = "fake".repeat(6);
    assert_eq!(scrub(&format!("bad key {key}!"), &key), "bad key \u{2026}!");
    let long = format!("{}{key}", "x".repeat(TEXT_MAX));
    let cut = scrub(&long, &key);
    assert!(!cut.contains(&key));
    assert_eq!(cut.chars().count(), TEXT_MAX + 1);
}

#[test]
fn a_request_is_named_by_its_endpoint_without_the_title() {
    assert_eq!(endpoint(&api::search("Sample Quest")), "search");
    assert_eq!(endpoint(&api::game("gog", "1")), "games");
}

#[test]
fn a_missing_key_or_one_not_of_a_keys_shape_is_refused_without_a_request() {
    for key in [
        "",
        "  \t",
        "short",
        "fake fake fake fake fake",
        "fake\r\nX-Other: 1fakefakefake",
    ] {
        let mut rig = steam_rig();
        rig.key = key.to_owned();
        assert_eq!(rig.steam("620", ALL), Err(Problem::Refused), "{key:?}");
        assert_eq!(rig.urls(), Vec::<String>::new());
    }
    let mut rig = steam_rig();
    rig.key = format!(" {} ", "fake".repeat(8));
    assert_eq!(names(&rig.steam("620", &[Kind::Square])).len(), 3);
}

#[test]
fn a_steam_game_is_asked_for_by_its_app_id_at_exact_addresses() {
    let mut rig = steam_rig();
    let found = rig.steam("620", ALL);
    assert_eq!(
        rig.urls(),
        [
            "https://www.steamgriddb.com/api/v2/grids/steam/620\
             ?dimensions=512x512,1024x1024&types=static&nsfw=false&humor=false",
            "https://www.steamgriddb.com/api/v2/grids/steam/620\
             ?dimensions=600x900,660x930&types=static&nsfw=false&humor=false",
            "https://www.steamgriddb.com/api/v2/heroes/steam/620\
             ?dimensions=1920x620&types=static&nsfw=false&humor=false",
        ]
    );
    assert_eq!(
        names(&found),
        [
            named(1104, "webp"),
            named(1103, "jpg"),
            named(1102, "png"),
            named(1203, "png"),
            named(1202, "png"),
            named(1201, "png"),
            named(1302, "png"),
            named(1301, "png"),
        ]
    );
}

#[test]
fn pictures_are_of_their_kind_and_say_whether_they_show_the_title() {
    let mut rig = steam_rig();
    let found = rig.steam("620", ALL).unwrap();
    let kinds = (found.iter())
        .map(|picked| (picked.kind, picked.titled))
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            (Kind::Square, true),
            (Kind::Square, true),
            (Kind::Square, false),
            (Kind::Cover, true),
            (Kind::Cover, true),
            (Kind::Cover, false),
            (Kind::Hero, false),
            (Kind::Hero, false),
        ]
    );
    for picked in &found {
        assert!(
            picked.url.starts_with("https://cdn2.steamgriddb.com/"),
            "{}",
            picked.url
        );
    }
}

#[test]
fn only_the_kinds_wanted_are_asked_for() {
    let mut rig = steam_rig();
    let found = rig.steam("620", &[Kind::Hero]);
    assert_eq!(rig.urls(), [hero("steam", "620")]);
    assert_eq!(names(&found), [named(1302, "png"), named(1301, "png")]);

    let mut rig = steam_rig();
    let _ = rig.steam("620", &[Kind::Cover, Kind::Square, Kind::Cover]);
    assert_eq!(rig.urls(), [cover("steam", "620"), square("steam", "620")]);

    let mut rig = steam_rig();
    assert_eq!(rig.steam("620", &[]), Ok(Vec::new()));
    assert_eq!(rig.urls(), Vec::<String>::new());
}

#[test]
fn pictures_are_ordered_by_score_then_upvotes_then_size_and_verified_plays_no_part() {
    let mut rig = Rig::new();
    let body = br#"{"success":true,"data":[
        {"id":1,"score":2,"upvotes":0,"width":512,"height":512,"verified":true,
         "url":"https://cdn2.steamgriddb.com/grid/1.png","mime":"image/png"},
        {"id":2,"score":3,"upvotes":0,"width":1024,"height":1024,"verified":false,
         "url":"https://cdn2.steamgriddb.com/grid/2.png","mime":"image/png"},
        {"id":3,"score":3,"upvotes":7,"width":1024,"height":1024,
         "url":"https://cdn2.steamgriddb.com/grid/3.png","mime":"image/png"},
        {"id":4,"score":3,"upvotes":7,"width":512,"height":512,
         "url":"https://cdn2.steamgriddb.com/grid/4.png","mime":"image/png"}
    ]}"#;
    rig.answer(&square("steam", "10"), answer(200, body, None));
    let found = rig.steam("10", &[Kind::Square]);
    assert_eq!(names(&found), ["4.png", "3.png", "2.png"]);
}

#[test]
fn nothing_of_any_kind_finds_nothing() {
    let mut rig = Rig::new();
    for url in [
        square("steam", "620"),
        cover("steam", "620"),
        hero("steam", "620"),
    ] {
        rig.answer(&url, ok("empty.json"));
    }
    assert_eq!(rig.steam("620", ALL), Ok(Vec::new()));
    assert_eq!(rig.urls().len(), 3);
}

#[test]
fn an_unknown_app_id_finds_nothing_after_one_request() {
    let mut rig = Rig::new();
    rig.answer(
        &square("steam", "999999"),
        answer(404, fixture("not-found.json"), None),
    );
    assert_eq!(rig.steam("999999", ALL), Ok(Vec::new()));
    assert_eq!(rig.urls(), [square("steam", "999999")]);
}

#[test]
fn a_kind_that_is_not_found_leaves_the_others() {
    let mut rig = Rig::new();
    rig.answer(&square("steam", "620"), ok("grids-square.json"));
    rig.answer(
        &cover("steam", "620"),
        answer(404, fixture("not-found.json"), None),
    );
    let found = rig.steam("620", &[Kind::Square, Kind::Cover]);
    assert_eq!(names(&found).len(), 3);
}

#[test]
fn errors_fail_the_lookup_with_their_text() {
    let mut rig = Rig::new();
    rig.answer(
        &square("steam", "620"),
        answer(400, fixture("errors.json"), None),
    );
    let text = failure(rig.steam("620", ALL));
    assert!(text.contains("400"), "{text}");
    assert!(text.contains("dimensions: must be one of"), "{text}");
    assert_eq!(rig.urls().len(), 1);

    let mut rig = Rig::new();
    rig.answer(
        &square("steam", "620"),
        answer(200, fixture("errors.json"), None),
    );
    let text = failure(rig.steam("620", ALL));
    assert!(text.contains("did not succeed"), "{text}");
}

#[test]
fn a_refused_key_is_refused() {
    for status in [401, 403] {
        let mut rig = steam_rig();
        rig.answer(
            &square("steam", "620"),
            answer(status, fixture("unauthorized.json"), None),
        );
        assert_eq!(rig.steam("620", ALL), Err(Problem::Refused), "{status}");
        assert_eq!(rig.urls().len(), 1);
    }
}

#[test]
fn too_many_requests_with_retry_after_is_later_by_that() {
    let mut rig = steam_rig();
    rig.answer(
        &square("steam", "620"),
        answer(429, fixture("too-many.json"), Some(120)),
    );
    assert_eq!(rig.steam("620", ALL), Err(Problem::Later(120)));
    assert_eq!(rig.urls().len(), 1);
}

#[test]
fn server_errors_are_later_and_retry_after_is_kept_within_a_second_and_an_hour() {
    for (status, retry_after, wait) in [
        (429, None, 60),
        (503, None, 60),
        (500, Some(30), 30),
        (502, Some(0), 1),
        (429, Some(99_999), 3600),
    ] {
        let mut rig = steam_rig();
        rig.answer(&square("steam", "620"), answer(status, b"", retry_after));
        assert_eq!(
            rig.steam("620", ALL),
            Err(Problem::Later(wait)),
            "{status} {retry_after:?}"
        );
    }
}

#[test]
fn a_retry_after_date_is_counted_from_the_answers_date_and_anything_else_is_the_first_wait() {
    let date = "Wed, 21 Oct 2015 07:28:00 GMT";
    for (retry_after, wait) in [
        ("Wed, 21 Oct 2015 08:28:00 GMT", 3600),
        ("Wed, 21 Oct 2015 07:30:00 GMT", 120),
        ("Wed, 21 Oct 2015 07:00:00 GMT", 1),
        ("Thu, 22 Oct 2015 07:28:00 GMT", LATER_MAX),
        ("soon", LATER_FIRST),
        ("-1", LATER_FIRST),
        ("", LATER_FIRST),
    ] {
        let mut rig = steam_rig();
        let mut busy = answer(503, b"", None);
        busy.headers.extend([
            ("date".to_owned(), date.to_owned()),
            ("retry-after".to_owned(), retry_after.to_owned()),
        ]);
        rig.answer(&square("steam", "620"), busy);
        assert_eq!(
            rig.steam("620", ALL),
            Err(Problem::Later(wait)),
            "{retry_after:?}"
        );
    }
}

#[test]
fn an_http_date_is_read_by_the_civil_calendar() {
    assert_eq!(http_date("Thu, 01 Jan 1970 00:00:10 GMT"), Some(10));
    assert_eq!(
        http_date("Sun, 06 Nov 1994 08:49:37 GMT"),
        Some(784_111_777)
    );
    assert_eq!(
        http_date("Tue, 29 Feb 2028 00:00:00 GMT"),
        Some(1_835_395_200)
    );
    for other in [
        "Sunday, 06-Nov-94 08:49:37 GMT",
        "Sun Nov  6 08:49:37 1994",
        "Sun, 06 Nov 1994 08:49:37 CET",
        "Sun, 06 Nov 1994 08:49 GMT",
        "Sun, 06 Nov 1994 99999999999999999:00:00 GMT",
        "Sun, 06 Nov 18446744073709551615 08:49:37 GMT",
        "Sun, 06 Nov 1994 24:00:00 GMT",
    ] {
        assert_eq!(http_date(other), None, "{other}");
    }
}

#[test]
fn a_wait_asked_for_midway_drops_what_was_found() {
    let mut rig = steam_rig();
    rig.answer(&cover("steam", "620"), answer(429, b"", Some(5)));
    assert_eq!(rig.steam("620", ALL), Err(Problem::Later(5)));
    assert_eq!(rig.urls().len(), 2);
}

#[test]
fn requests_within_a_call_start_at_least_a_second_apart() {
    let mut rig = steam_rig();
    let _ = rig.steam("620", ALL);
    assert_eq!(rig.clock.slept, [PACE, PACE]);
    let _ = rig.steam("620", &[Kind::Square]);
    assert_eq!(
        rig.clock.slept,
        [PACE, PACE],
        "a call's first request does not wait: Kidō paces the calls"
    );
    let _ = rig.steam("620", &[]);
    assert_eq!(
        rig.clock.slept.len(),
        2,
        "a call without a request does not wait"
    );
}

#[test]
fn the_network_failing_fails_the_lookup() {
    let mut rig = steam_rig();
    rig.fail(
        &square("steam", "620"),
        HttpError::Unreachable("the connection timed out".to_owned()),
    );
    let text = failure(rig.steam("620", ALL));
    assert!(text.contains("timed out"), "{text}");
    rig.fail(&square("steam", "620"), HttpError::TooLarge);
    let text = failure(rig.steam("620", ALL));
    assert!(text.contains("larger than Kidō reads"), "{text}");
    rig.fail(&square("steam", "620"), HttpError::Denied);
    let text = failure(rig.steam("620", ALL));
    assert!(text.contains("denied"), "{text}");
    assert_eq!(rig.urls().len(), 3, "each lookup asked");
}

#[test]
fn an_answer_that_is_not_json_fails() {
    let mut rig = steam_rig();
    rig.answer(
        &square("steam", "620"),
        answer(
            200,
            b"<html><body>Sign in to the network</body></html>",
            None,
        ),
    );
    let text = failure(rig.steam("620", ALL));
    assert!(text.contains("not readable"), "{text}");
}

#[test]
fn a_gog_game_is_found_by_its_id_and_its_art_by_steamgriddbs_own() {
    let mut rig = Rig::new();
    rig.answer(&format!("{API}/games/gog/1207658924"), ok("game-gog.json"));
    rig.answer(&square("game", "5247"), ok("grids-square.json"));
    let found = rig.gog("1207658924", "Another Title", &[Kind::Square]);
    assert_eq!(names(&found).len(), 3);
    assert_eq!(
        rig.urls(),
        [
            "https://www.steamgriddb.com/api/v2/games/gog/1207658924".to_owned(),
            "https://www.steamgriddb.com/api/v2/grids/game/5247\
             ?dimensions=512x512,1024x1024&types=static&nsfw=false&humor=false"
                .to_owned(),
        ]
    );
}

#[test]
fn a_gog_id_it_does_not_know_is_searched_for_by_its_exact_title() {
    for (status, body) in [
        (404, fixture("not-found.json")),
        (400, fixture("errors.json")),
        (422, &b""[..]),
        (200, fixture("not-found.json")),
    ] {
        let mut rig = Rig::new();
        rig.answer(
            &format!("{API}/games/gog/1207658924"),
            answer(status, body, None),
        );
        rig.answer(
            &format!("{API}/search/autocomplete/Sample%20Quest"),
            ok("search.json"),
        );
        rig.answer(&cover("game", "903"), ok("grids-cover.json"));
        let found = rig.gog("1207658924", "Sample Quest\u{2122}", &[Kind::Cover]);
        assert_eq!(names(&found).len(), 3, "{status}");
        assert_eq!(
            rig.urls(),
            [
                "https://www.steamgriddb.com/api/v2/games/gog/1207658924".to_owned(),
                "https://www.steamgriddb.com/api/v2/search/autocomplete/Sample%20Quest".to_owned(),
                "https://www.steamgriddb.com/api/v2/grids/game/903\
                 ?dimensions=600x900,660x930&types=static&nsfw=false&humor=false"
                    .to_owned(),
            ],
            "{status}"
        );
    }
}

#[test]
fn a_title_that_is_not_exactly_found_finds_nothing() {
    let mut rig = Rig::new();
    rig.answer(
        &format!("{API}/search/autocomplete/Sample%20Quest%20Deluxe"),
        ok("search.json"),
    );
    assert_eq!(rig.gog("1", "Sample Quest Deluxe", ALL), Ok(Vec::new()));
    assert_eq!(rig.urls().len(), 2, "no art is asked for");

    let mut rig = Rig::new();
    assert_eq!(rig.gog("1", "\u{2122}", ALL), Ok(Vec::new()));
    assert_eq!(rig.urls().len(), 1, "nothing to search for");
}

#[test]
fn a_store_without_a_platform_is_searched_for_by_title_alone() {
    let mut rig = Rig::new();
    rig.answer(
        &format!("{API}/search/autocomplete/Sample%20Quest"),
        ok("search.json"),
    );
    rig.answer(&hero("game", "903"), ok("heroes.json"));
    let found = rig.look_up("epic", "sq", "Sample Quest", &[Kind::Hero]);
    assert_eq!(names(&found), [named(1302, "png"), named(1301, "png")]);
    assert_eq!(rig.urls().len(), 2);
}

#[test]
fn a_refused_key_or_a_wait_while_finding_a_gog_game_ends_the_lookup() {
    let mut rig = Rig::new();
    rig.answer(
        &format!("{API}/games/gog/1"),
        answer(401, fixture("unauthorized.json"), None),
    );
    assert_eq!(rig.gog("1", "Sample Quest", ALL), Err(Problem::Refused));
    assert_eq!(rig.urls().len(), 1);

    let mut rig = Rig::new();
    rig.answer(&format!("{API}/games/gog/1"), ok("not-found.json"));
    rig.answer(
        &format!("{API}/search/autocomplete/Sample%20Quest"),
        answer(429, fixture("too-many.json"), Some(120)),
    );
    assert_eq!(rig.gog("1", "Sample Quest", ALL), Err(Problem::Later(120)));
}

#[test]
fn the_key_goes_only_to_the_api_host_in_the_authorization_header() {
    let mut rig = steam_rig();
    rig.answer(&format!("{API}/games/gog/1"), answer(404, b"", None));
    rig.answer(
        &format!("{API}/search/autocomplete/Sample%20Quest"),
        ok("search.json"),
    );
    rig.answer(&square("game", "903"), ok("grids-square.json"));
    let found = [
        rig.steam("620", ALL),
        rig.gog("1", "Sample Quest", &[Kind::Square]),
    ];
    let key = rig.key.clone();
    let bearer = format!("Bearer {key}");
    assert_eq!(rig.http.asked.len(), 6);
    for (url, headers) in &rig.http.asked {
        let rest = url.strip_prefix("https://").unwrap();
        assert_eq!(rest.split('/').next(), Some("www.steamgriddb.com"));
        assert!(url.starts_with(&format!("{API}/")), "{url}");
        assert!(!url.contains(&key));
        let carrying = (headers.iter())
            .filter(|(_, value)| value.contains(&key))
            .collect::<Vec<_>>();
        assert_eq!(carrying, [&("Authorization".to_owned(), bearer.clone())]);
    }
    for found in &found {
        for picked in found.as_ref().unwrap() {
            assert!(!picked.url.contains(&key));
            assert!(
                !rig.http.asked.iter().any(|(url, _)| *url == picked.url),
                "a picture was fetched with the key: {}",
                picked.url
            );
        }
    }
}

#[test]
fn no_problem_holds_the_key() {
    let mut rig = Rig::new();
    let key = rig.key.clone();
    let echo = format!(r#"{{"success":false,"errors":["the key {key} is not valid"]}}"#);
    rig.answer(&square("steam", "3"), answer(400, echo.as_bytes(), None));
    rig.answer(&square("steam", "4"), answer(200, echo.as_bytes(), None));
    rig.fail(
        &square("steam", "5"),
        HttpError::Failed(format!("the proxy refused Bearer {key}")),
    );
    rig.answer(
        &format!("{API}/games/gog/8"),
        answer(500, echo.as_bytes(), None),
    );
    for found in [
        rig.steam("3", ALL),
        rig.steam("4", ALL),
        rig.steam("5", ALL),
        rig.gog("8", "Sample Quest", &[Kind::Square]),
    ] {
        let text = match found {
            Err(Problem::Failed(text)) => text,
            Err(Problem::Later(_)) => continue,
            found => panic!("did not fail: {found:?}"),
        };
        assert!(!text.contains(&key), "a problem holds the key: {text}");
    }
}

/// `n` damaged copies of `bytes`, the same for the same `seed`: bits flipped, cut short, a part
/// copied over another, a block repeated, or a number at a bound.
fn mutate(seed: u64, bytes: &[u8], n: usize) -> Vec<Vec<u8>> {
    let mut state = seed;
    let mut next = move |bound: usize| {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        usize::try_from((z ^ (z >> 31)) % u64::try_from(bound).unwrap()).unwrap()
    };
    (0..n)
        .map(|_| {
            let mut copy = bytes.to_vec();
            let len = copy.len();
            if len == 0 {
                return (0..=next(8)).map(|_| next(256).to_le_bytes()[0]).collect();
            }
            match next(5) {
                0 => {
                    for _ in 0..=next(8) {
                        let at = next(len);
                        copy[at] ^= 1 << next(8);
                    }
                }
                1 => copy.truncate(next(len)),
                2 => {
                    let (from, to) = (next(len), next(len));
                    let size = 1 + next(len - from.max(to));
                    copy.copy_within(from..from + size, to);
                }
                3 => {
                    let from = next(len);
                    let size = 1 + next((len - from).min(4096));
                    let block = copy[from..from + size].to_vec();
                    copy.splice(from + size..from + size, block);
                }
                _ => {
                    let width = [1, 2, 4, 8][next(4)].min(len);
                    let at = next(len - width + 1);
                    let top = 1u64 << (8 * width - 1);
                    let bound = [0, u64::MAX, top - 1, top][next(4)].to_le_bytes();
                    copy[at..at + width].copy_from_slice(&bound[..width]);
                }
            }
            copy
        })
        .collect()
}

#[test]
fn mutants_of_every_fixture_are_read_without_a_panic() {
    for (at, (_, bytes)) in FIXTURES.iter().enumerate() {
        let seed = 0x5347_4442_0000 + u64::try_from(at).unwrap();
        for mutant in mutate(seed, bytes, 1_000) {
            if let Ok(images) = json::images(&mutant) {
                for &kind in ALL {
                    pick(kind, &images);
                }
            }
            if let Ok(found) = json::games(&mutant) {
                title::choose("Sample Quest", &found);
            }
            let _ = json::game(&mutant);
            let _ = json::errors(&mutant);
        }
    }
}

#[test]
fn mutants_as_answers_end_every_lookup_without_a_panic() {
    for (at, (_, bytes)) in FIXTURES.iter().enumerate() {
        let seed = 0x4c4f_4f4b_0000 + u64::try_from(at).unwrap();
        for (turn, mutant) in mutate(seed, bytes, 300).into_iter().enumerate() {
            let status = [200, 200, 200, 404, 400, 429, 503][turn % 7];
            let mut rig = Rig::new();
            for url in [
                format!("{API}/grids/steam/1{SQUARES}"),
                format!("{API}/grids/steam/1{COVERS}"),
                format!("{API}/heroes/steam/1{HEROES}"),
                format!("{API}/games/gog/1"),
                format!("{API}/search/autocomplete/Sample%20Quest"),
                format!("{API}/grids/game/903{SQUARES}"),
            ] {
                rig.answer(&url, answer(status, &mutant, Some(1)));
            }
            let found = [
                rig.steam("1", ALL),
                rig.gog("1", "Sample Quest", &[Kind::Square]),
            ];
            for picked in found.iter().flatten().flatten() {
                assert!(picked.url.starts_with("https://"), "{}", picked.url);
            }
        }
    }
}
