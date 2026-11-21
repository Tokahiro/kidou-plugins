use super::{
    AGENT, Page, found_page, id_field, id_lookup, page, parsed, same_title, shop, wikitext,
};

#[test]
fn steam_and_gog_alone_have_an_id_field() {
    assert_eq!(id_field("steam"), Some("steamappid"));
    assert_eq!(id_field("gog"), Some("gogid"));
    assert_eq!(id_field("epic"), None);
    assert_eq!(id_field("Steam"), None);
}

#[test]
fn an_id_of_digits_alone_is_sent() {
    assert_eq!(
        id_lookup("steamappid", "1145350").as_deref(),
        Some(
            "https://www.pcgamingwiki.com/w/api.php?action=idlookup&field=steamappid\
             &value=1145350&format=json&formatversion=2"
        )
    );
    for wrong in ["", "12a", "1&field=gogid", "123456789012345678901"] {
        assert_eq!(id_lookup("steamappid", wrong), None, "{wrong:?}");
    }
}

#[test]
fn a_title_is_percent_encoded_and_a_title_of_no_page_never_sent() {
    assert_eq!(
        wikitext(" Hades II ").as_deref(),
        Some(
            "https://www.pcgamingwiki.com/w/api.php?action=parse&page=Hades%20II\
             &prop=wikitext&redirects=1&format=json&formatversion=2"
        )
    );
    let encoded = wikitext("Ōkami HD & more?").unwrap();
    assert!(
        encoded.contains("page=%C5%8Ckami%20HD%20%26%20more%3F&"),
        "{encoded}"
    );
    for wrong in [
        "",
        "  ",
        "A|B",
        "A{{B}}",
        "A#B",
        "line\nbreak",
        "x".repeat(256).as_str(),
    ] {
        assert_eq!(wikitext(wrong), None, "{wrong:?}");
    }
}

#[test]
fn the_credit_names_the_page_by_its_id() {
    assert_eq!(page(183_626), "pcgamingwiki.com/?curid=183626");
}

#[test]
fn the_id_lookup_names_the_page_or_none() {
    // Recorded 2026-11-21.
    let hades = br#"{"idlookup":[{"title":{"Page":"Hades II"}}]}"#;
    assert_eq!(found_page(hades), Ok(Some("Hades II".to_owned())));
    assert_eq!(found_page(br#"{"idlookup":[]}"#), Ok(None));
    assert_eq!(
        found_page(br#"{"idlookup":[{"title":{"Page":" "}}]}"#),
        Ok(None)
    );
    assert!(found_page(b"<html>Just a moment...</html>").is_err());
    assert_eq!(
        found_page(br#"{"error":{"code":"ratelimited","info":"Too fast."}}"#),
        Err("PCGamingWiki answered ratelimited".to_owned())
    );
}

#[test]
fn a_parse_gives_the_page_none_or_the_wiki_s_error() {
    let hades =
        br#"{"parse":{"title":"Hades II","pageid":183626,"wikitext":"{{Video\n|hdr = false\n}}"}}"#;
    assert_eq!(
        parsed(hades),
        Ok(Some(Page {
            title: "Hades II".to_owned(),
            id: 183_626,
            wikitext: "{{Video\n|hdr = false\n}}".to_owned(),
        }))
    );
    // Recorded 2026-11-21, the docref cut.
    let missing = br#"{"error":{"code":"missingtitle","info":"The page you specified doesn't exist.","docref":"See https://www.pcgamingwiki.com/w/api.php for API usage."}}"#;
    assert_eq!(parsed(missing), Ok(None));
    let other = br#"{"error":{"code":"ratelimited","info":"Too fast."}}"#;
    assert_eq!(
        parsed(other),
        Err("PCGamingWiki answered ratelimited".to_owned())
    );
    assert!(parsed(br#"{"batchcomplete":true}"#).is_err());
    assert!(parsed(b"").is_err());
}

#[test]
fn the_agent_names_the_manifest_s_version() {
    let manifest = include_str!("../../plugin.toml");
    let version = (manifest.lines())
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap();
    assert!(
        AGENT.starts_with(&format!("Kidou-PCGamingWiki/{version} ")),
        "{AGENT}"
    );
}

#[test]
fn epic_and_xbox_games_are_found_by_title_and_store() {
    assert_eq!(shop("epic"), Some("Epic Games Store"));
    assert_eq!(shop("xbox"), Some("Microsoft Store"));
    assert_eq!(shop("steam"), None);
    assert_eq!(shop("itch"), None);
}

#[test]
fn titles_match_but_for_case_spaces_and_punctuation() {
    assert!(same_title("Hades II", "HADES  II"));
    assert!(same_title("Hades II", "Hades: II"));
    assert!(same_title("Ōkami HD", "ōkami hd"));
    assert!(!same_title("Hades", "Hades II"));
    assert!(!same_title("Tomb Raider (1996)", "Tomb Raider"));
    assert!(!same_title("", ""));
    assert!(!same_title("!!", "??"));
}
