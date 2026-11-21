//! The asks against the wiki's answers as a fake gives them: the page found by the store's id
//! or by the title, its facts read, and every answer that is no page. The wiki itself is asked
//! only on the device: its firewall refuses the cloud's networks.

use std::collections::{HashMap, VecDeque};

use super::{Answer, Ask, Get, LATER_BUSY, LATER_REFUSED, Language, Problem, Unsent, about};

const HADES: &str = include_str!("answers/hades-ii.wikitext");

const LOOKUP_STEAM: &str = "https://www.pcgamingwiki.com/w/api.php?action=idlookup\
                            &field=steamappid&value=1145350&format=json&formatversion=2";
const LOOKUP_GOG: &str = "https://www.pcgamingwiki.com/w/api.php?action=idlookup\
                          &field=gogid&value=1423049311&format=json&formatversion=2";
const PAGE_HADES: &str = "https://www.pcgamingwiki.com/w/api.php?action=parse\
                          &page=Hades%20II&prop=wikitext&redirects=1&format=json&formatversion=2";
const PAGE_CYBERPUNK: &str = "https://www.pcgamingwiki.com/w/api.php?action=parse\
                              &page=Cyberpunk%202077&prop=wikitext&redirects=1&format=json\
                              &formatversion=2";

/// Gives each URL's answers in turn, and a 404 once they run out.
#[derive(Default)]
struct FakeHttp {
    answers: HashMap<&'static str, VecDeque<Result<Answer, Unsent>>>,
    asked: Vec<(String, Vec<(String, String)>)>,
}

impl FakeHttp {
    fn answer(&mut self, url: &'static str, answer: Result<Answer, Unsent>) {
        self.answers.entry(url).or_default().push_back(answer);
    }

    fn urls(&self) -> Vec<&str> {
        self.asked.iter().map(|(url, _)| url.as_str()).collect()
    }
}

impl Get for FakeHttp {
    fn get(&mut self, url: &str, headers: &[(&str, &str)]) -> Result<Answer, Unsent> {
        self.asked.push((
            url.to_owned(),
            (headers.iter())
                .map(|&(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
        ));
        let next = self.answers.get_mut(url).and_then(VecDeque::pop_front);
        next.unwrap_or_else(|| status(404, &[]))
    }
}

#[expect(clippy::unnecessary_wraps, reason = "an answer as the fake gives it")]
fn ok(body: &str) -> Result<Answer, Unsent> {
    Ok(Answer {
        status: 200,
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
    })
}

#[expect(clippy::unnecessary_wraps, reason = "an answer as the fake gives it")]
fn status(status: u16, headers: &[(&str, &str)]) -> Result<Answer, Unsent> {
    Ok(Answer {
        status,
        headers: (headers.iter())
            .map(|&(name, value)| (name.to_owned(), value.to_owned()))
            .collect(),
        body: Vec::new(),
    })
}

/// The answer of `parse` for Hades II, around the recorded wikitext.
fn hades_page() -> String {
    serde_json::json!({ "parse": { "title": "Hades II", "pageid": 183_626, "wikitext": HADES } })
        .to_string()
}

fn ask<'a>(store: &'a str, store_id: &'a str, title: &'a str) -> Ask<'a> {
    Ask {
        store,
        store_id,
        title,
        facts: true,
        language: Language::English,
    }
}

#[test]
fn a_steam_game_is_found_by_its_id_and_its_page_read() {
    let mut http = FakeHttp::default();
    http.answer(
        LOOKUP_STEAM,
        ok(r#"{"idlookup":[{"title":{"Page":"Hades II"}}]}"#),
    );
    http.answer(PAGE_HADES, ok(&hades_page()));
    let rows = about(&mut http, &ask("steam", "1145350", "Hades 2")).unwrap();
    assert_eq!(rows.first().map(|row| row.value.as_str()), Some("Full"));
    assert_eq!(rows.len(), 7);
    assert_eq!(http.urls(), [LOOKUP_STEAM, PAGE_HADES]);
    for (_, headers) in &http.asked {
        assert!(
            headers.iter().any(|(name, value)| name == "User-Agent"
                && value.starts_with("Kidou-PCGamingWiki/")
                && value.contains("https://github.com/Tokahiro/kidou-plugins")),
            "{headers:?}"
        );
    }
}

#[test]
fn a_gog_game_is_found_by_its_id() {
    let mut http = FakeHttp::default();
    // Recorded 2026-11-21.
    http.answer(
        LOOKUP_GOG,
        ok(r#"{"idlookup":[{"title":{"Page":"Cyberpunk 2077"}}]}"#),
    );
    let page = r#"{"parse":{"title":"Cyberpunk 2077","wikitext":"{{Video\n|hdr = true\n}}"}}"#;
    http.answer(PAGE_CYBERPUNK, ok(page));
    let rows = about(&mut http, &ask("gog", "1423049311", "Cyberpunk 2077")).unwrap();
    assert_eq!(rows.first().map(|row| row.value.as_str()), Some("Yes"));
    assert_eq!(http.urls(), [LOOKUP_GOG, PAGE_CYBERPUNK]);
}

#[test]
fn a_game_of_another_store_is_found_by_its_title() {
    let mut http = FakeHttp::default();
    http.answer(PAGE_HADES, ok(&hades_page()));
    let rows = about(&mut http, &ask("xbox", "9PCR3Z2MSM4T", "Hades II")).unwrap();
    assert_eq!(rows.len(), 7);
    assert_eq!(http.urls(), [PAGE_HADES]);
}

#[test]
fn nothing_is_asked_unless_the_page_shows_or_for_an_id_that_is_no_number() {
    let mut http = FakeHttp::default();
    let art_only = Ask {
        facts: false,
        ..ask("steam", "1145350", "Hades II")
    };
    assert_eq!(about(&mut http, &art_only), Ok(Vec::new()));
    assert_eq!(
        about(&mut http, &ask("steam", "x1", "Hades II")),
        Ok(Vec::new())
    );
    assert_eq!(about(&mut http, &ask("epic", "a", "A|B")), Ok(Vec::new()));
    assert_eq!(http.asked, Vec::new());
}

#[test]
fn a_game_the_wiki_does_not_know_has_no_facts() {
    let mut http = FakeHttp::default();
    http.answer(LOOKUP_STEAM, ok(r#"{"idlookup":[]}"#));
    assert_eq!(
        about(&mut http, &ask("steam", "1145350", "Hades II")),
        Ok(Vec::new())
    );
    assert_eq!(http.urls(), [LOOKUP_STEAM]);

    http.answer(
        PAGE_HADES,
        ok(r#"{"error":{"code":"missingtitle","info":"The page you specified doesn't exist."}}"#),
    );
    assert_eq!(
        about(&mut http, &ask("epic", "x", "Hades II")),
        Ok(Vec::new())
    );
}

#[test]
fn the_wiki_is_left_alone_when_busy_down_or_refusing() {
    let hades = ask("steam", "1145350", "Hades II");
    for (answer, later) in [
        (status(429, &[("Retry-After", "120")]), 120),
        (status(429, &[("retry-after", "99999")]), LATER_REFUSED),
        (
            status(429, &[("Retry-After", "Wed, 21 Oct 2026 07:28:00 GMT")]),
            LATER_BUSY,
        ),
        (status(429, &[]), LATER_BUSY),
        (status(503, &[]), LATER_BUSY),
        (status(403, &[]), LATER_REFUSED),
    ] {
        let mut http = FakeHttp::default();
        http.answer(LOOKUP_STEAM, answer);
        assert_eq!(about(&mut http, &hades), Err(Problem::Later(later)));
        assert_eq!(http.urls().len(), 1);
    }
}

#[test]
fn anything_else_fails_for_the_log() {
    let hades = ask("steam", "1145350", "Hades II");
    for (answer, text) in [
        (status(404, &[]), "PCGamingWiki answered HTTP 404"),
        (ok("<html>"), "PCGamingWiki's id lookup does not read"),
        (Err(Unsent::Denied), "Kidō denied the request"),
        (
            Err(Unsent::TooLarge),
            "the answer is larger than Kidō reads",
        ),
        (
            Err(Unsent::Unreachable("offline".to_owned())),
            "no network: offline",
        ),
        (Err(Unsent::Failed("reset".to_owned())), "reset"),
    ] {
        let mut http = FakeHttp::default();
        http.answer(LOOKUP_STEAM, answer);
        assert_eq!(
            about(&mut http, &hades),
            Err(Problem::Failed(text.to_owned()))
        );
    }
}
