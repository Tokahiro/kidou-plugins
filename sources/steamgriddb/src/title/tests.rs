use super::*;

fn game(id: u64, name: &str, verified: bool) -> Game {
    Game {
        id,
        name: name.to_owned(),
        types: vec!["gog".to_owned()],
        verified,
    }
}

#[test]
fn titles_are_compared_without_case_signs_or_extra_space() {
    for (title, same) in [
        (
            "The Witcher\u{00ae} 3: Wild Hunt",
            "the witcher 3 wild hunt",
        ),
        (
            "  THE   WITCHER 3 - Wild Hunt\u{2122} ",
            "the witcher 3 wild hunt",
        ),
        ("Baldur\u{2019}s Gate", "baldurs gate"),
        ("Baldur's Gate", "baldurs gate"),
        (
            "S.T.A.L.K.E.R.: Shadow of Chernobyl",
            "stalker shadow of chernobyl",
        ),
        ("Half-Life 2", "half life 2"),
        ("Caf\u{e9}", "caf\u{e9}"),
        ("Cafe\u{301}", "cafe"),
        ("\u{2122}", ""),
    ] {
        assert_eq!(normalized(title), same, "{title}");
    }
}

#[test]
fn only_the_same_title_is_taken_and_a_verified_one_first() {
    let found = [
        game(901, "Sample Quest: Remastered", true),
        game(902, "Sample Quest", false),
        game(903, "SAMPLE QUEST\u{2122}", true),
        game(904, "Sample Quest", true),
    ];
    assert_eq!(choose("Sample Quest", &found), Some(903));
    assert_eq!(choose("Sample Quest", &found[..2]), Some(902));
    assert_eq!(choose("Sample Quest Remastered", &found), Some(901));
    assert_eq!(choose("Sample", &found), None);
    assert_eq!(choose("Sample Quest 2", &found), None);
}

#[test]
fn verified_does_not_make_another_title_the_same() {
    let found = [game(1, "Sample Quest Deluxe", true)];
    assert_eq!(choose("Sample Quest", &found), None);
}

#[test]
fn an_empty_title_matches_nothing() {
    let found = [game(1, "\u{2122}", true), game(2, "", true)];
    assert_eq!(choose("", &found), None);
    assert_eq!(choose(" \u{00ae} ", &found), None);
}
