use super::*;

/// The host of an `https` address: what lies between the scheme and the first `/`, `?` or
/// `#`.
fn host(url: &str) -> &str {
    let rest = url.strip_prefix("https://").unwrap();
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn requests_have_exact_paths_and_queries() {
    let square = art(Kind::Square, "steam", "620");
    assert_eq!(square.path, "/grids/steam/620");
    assert_eq!(
        square.url(),
        "https://www.steamgriddb.com/api/v2/grids/steam/620\
             ?dimensions=512x512,1024x1024&types=static&nsfw=false&humor=false"
    );
    assert_eq!(
        art(Kind::Cover, "game", "5247").url(),
        "https://www.steamgriddb.com/api/v2/grids/game/5247\
             ?dimensions=600x900,660x930&types=static&nsfw=false&humor=false"
    );
    assert_eq!(
        art(Kind::Hero, "steam", "620").url(),
        "https://www.steamgriddb.com/api/v2/heroes/steam/620\
             ?dimensions=1920x620&types=static&nsfw=false&humor=false"
    );
    assert_eq!(
        art(Kind::Logo, "steam", "620").url(),
        "https://www.steamgriddb.com/api/v2/logos/steam/620?types=static&nsfw=false&humor=false"
    );
    assert_eq!(
        game("gog", "1207658924").url(),
        "https://www.steamgriddb.com/api/v2/games/gog/1207658924"
    );
    assert_eq!(
        search("The Witcher\u{00ae} 3:  Wild Hunt\u{2122}").url(),
        "https://www.steamgriddb.com/api/v2/search/autocomplete/\
             The%20Witcher%203%3A%20Wild%20Hunt"
    );
}

#[test]
fn nothing_in_a_request_leaves_the_api_host() {
    let hostile = [
        "..",
        ".",
        "../../../x",
        "a/b?c=d#e",
        "@evil.example",
        "//evil.example/x",
        "x\\y",
        "%2e%2e",
        "caf\u{e9} \u{1f3ae}",
    ];
    for text in hostile {
        let urls = [
            art(Kind::Square, text, text).url(),
            game(text, text).url(),
            search(text).url(),
        ];
        for url in urls {
            assert_eq!(host(&url), "www.steamgriddb.com", "{url}");
            assert!(url.starts_with(&format!("{BASE}/")), "{url}");
            let path = url[BASE.len()..].split('?').next().unwrap();
            assert!(!path.contains(['#', '\\', '@']), "{url}");
            assert!(
                path.split('/')
                    .skip(1)
                    .all(|part| !part.is_empty() && part != "." && part != ".."),
                "{url}"
            );
        }
    }
}

#[test]
fn a_title_of_only_signs_and_space_leaves_nothing_to_search_for() {
    assert_eq!(search_term(" \u{2122} \t\u{00ae} "), "");
    assert_eq!(search_term(""), "");
    assert_eq!(search_term(" Half-Life\u{2122}  2 "), "Half-Life 2");
}

#[test]
fn every_kind_but_the_logo_has_sizes_and_the_preferred_first() {
    assert_eq!(dimensions(Kind::Square)[0], (512, 512));
    assert_eq!(dimensions(Kind::Cover)[0], (600, 900));
    assert_eq!(dimensions(Kind::Hero), [(1920, 620)]);
    assert_eq!(dimensions(Kind::Logo), []);
}
