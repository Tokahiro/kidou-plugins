use super::{Language, Row, rows};

/// Hades II's page as the wiki gave it on 2026-11-21, shortened to the templates the facts come
/// from and a few around them.
const HADES: &str = include_str!("../answers/hades-ii.wikitext");

fn told(rows: &[Row]) -> Vec<(&str, &str)> {
    (rows.iter())
        .map(|row| (row.label.as_str(), row.value.as_str()))
        .collect()
}

#[test]
fn hades_ii_s_facts_in_english_and_german() {
    assert_eq!(
        told(&rows(183_626, HADES, Language::English)),
        [
            ("Controller", "Full"),
            ("Frame rate", "Over 60 fps"),
            ("Ultrawide", "Yes"),
            ("HDR", "No"),
            ("Cloud saves", "Steam, Epic, Xbox"),
            ("Engine", "The Forge"),
            (
                "License",
                "CC BY-NC-SA 3.0, shortened, pcgamingwiki.com/?curid=183626"
            ),
        ]
    );
    assert_eq!(
        told(&rows(183_626, HADES, Language::German)),
        [
            ("Controller", "Vollständig"),
            ("Bildrate", "Über 60 fps"),
            ("Ultrawide", "Ja"),
            ("HDR", "Nein"),
            ("Cloud-Spielstände", "Steam, Epic, Xbox"),
            ("Engine", "The Forge"),
            (
                "Lizenz",
                "CC BY-NC-SA 3.0, gekürzt, pcgamingwiki.com/?curid=183626"
            ),
        ]
    );
}

#[test]
fn a_page_without_facts_gets_no_credit() {
    assert_eq!(
        rows(1, "{{Video\n|hdr = unknown\n}}", Language::English),
        Vec::new()
    );
    assert_eq!(
        rows(1, "#REDIRECT [[Hades II]]", Language::German),
        Vec::new()
    );
}

fn one(page: &str, language: Language) -> Vec<(String, String)> {
    let rows = rows(1, page, language);
    (rows.into_iter())
        .filter(|row| row.label != "License" && row.label != "Lizenz")
        .map(|row| (row.label, row.value))
        .collect()
}

fn row(label: &str, value: &str) -> Vec<(String, String)> {
    vec![(label.to_owned(), value.to_owned())]
}

#[test]
fn a_controller_is_taken_fully_partly_with_a_fix_or_not() {
    let input = |support: &str, full: &str| {
        format!("{{{{Input\n|controller support = {support}\n|full controller = {full}\n}}}}")
    };
    let en = Language::English;
    assert_eq!(one(&input("true", "true"), en), row("Controller", "Full"));
    assert_eq!(
        one(&input("true", "false"), en),
        row("Controller", "Partial")
    );
    assert_eq!(one(&input("true", ""), en), row("Controller", "Yes"));
    assert_eq!(
        one(&input("limited", "true"), en),
        row("Controller", "Partial")
    );
    assert_eq!(
        one(&input("hackable", "false"), en),
        row("Controller", "With a fix")
    );
    assert_eq!(one(&input("false", "false"), en), row("Controller", "No"));
    assert_eq!(
        one(&input("true", "false"), Language::German),
        row("Controller", "Teilweise")
    );
    assert_eq!(one(&input("unknown", "true"), en), Vec::new());
}

#[test]
fn the_frame_rate_is_over_up_to_or_under_60() {
    let video = |sixty: &str, above: &str| {
        format!("{{{{Video\n|60 fps = {sixty}\n|120 fps = {above}\n}}}}")
    };
    let en = Language::English;
    let rate = |value| row("Frame rate", value);
    assert_eq!(one(&video("true", "true"), en), rate("Over 60 fps"));
    assert_eq!(one(&video("unknown", "limited"), en), rate("Over 60 fps"));
    assert_eq!(
        one(&video("true", "hackable"), en),
        rate("Over 60 fps with a fix")
    );
    assert_eq!(one(&video("true", "false"), en), rate("Up to 60 fps"));
    assert_eq!(one(&video("true", "unknown"), en), rate("60 fps"));
    assert_eq!(one(&video("false", "false"), en), rate("Under 60 fps"));
    assert_eq!(
        one(&video("hackable", "false"), en),
        rate("60 fps with a fix")
    );
    assert_eq!(one(&video("unknown", "false"), en), Vec::new());
    assert_eq!(
        one(&video("true", "false"), Language::German),
        row("Bildrate", "Bis 60 fps")
    );
}

#[test]
fn ultrawide_and_hdr_take_the_wiki_s_words() {
    let video = "{{Video\n|ultrawidescreen = hackable\n|hdr = always on\n}}";
    assert_eq!(
        one(video, Language::German),
        [
            ("Ultrawide".to_owned(), "Mit Fix".to_owned()),
            ("HDR".to_owned(), "Immer an".to_owned()),
        ]
    );
    let video = "{{Video\n|ultrawidescreen = limited\n|hdr = n/a\n}}";
    assert_eq!(one(video, Language::English), row("Ultrawide", "Limited"));
}

#[test]
fn cloud_saves_name_their_services_or_say_no() {
    let clouds = |steam: &str, gog: &str| {
        format!(
            "{{{{Save game cloud syncing\n|steam cloud = {steam}\n|gog galaxy = {gog}\n|origin = \n}}}}"
        )
    };
    let en = Language::English;
    assert_eq!(
        one(&clouds("true", "true"), en),
        row("Cloud saves", "Steam, GOG")
    );
    assert_eq!(one(&clouds("false", "true"), en), row("Cloud saves", "GOG"));
    assert_eq!(one(&clouds("false", ""), en), row("Cloud saves", "No"));
    assert_eq!(
        one(&clouds("false", "unknown"), Language::German),
        row("Cloud-Spielstände", "Nein")
    );
    assert_eq!(one(&clouds("unknown", ""), en), Vec::new());
}

#[test]
fn at_most_two_engines_are_named_by_their_text() {
    let page = "{{Infobox game\n|engines =\n\
                {{Infobox game/row/engine|[[Engine:REDengine 4|REDengine 4]]}}\n\
                {{Infobox game/row/engine|Havok}}\n\
                {{Infobox game/row/engine|Wwise}}\n}}";
    assert_eq!(
        one(page, Language::English),
        row("Engine", "REDengine 4, Havok")
    );
}
