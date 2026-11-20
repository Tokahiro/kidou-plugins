use super::*;

fn image(id: u64, score: i64, upvotes: i64, size: (u32, u32)) -> Image {
    Image {
        id,
        score,
        style: "alternate".to_owned(),
        width: size.0,
        height: size.1,
        nsfw: false,
        humor: false,
        epilepsy: false,
        mime: "image/png".to_owned(),
        url: format!("https://cdn.example/{id}.png"),
        upvotes,
        downvotes: 0,
    }
}

fn ids(picked: &[Picked]) -> Vec<String> {
    picked
        .iter()
        .map(|picked| {
            picked
                .url
                .trim_start_matches("https://cdn.example/")
                .to_owned()
        })
        .collect()
}

#[test]
fn score_first_then_upvotes_then_the_preferred_size() {
    let images = [
        image(1, 5, 9, (512, 512)),
        image(2, 9, 1, (1024, 1024)),
        image(3, 9, 4, (1024, 1024)),
        image(4, 9, 4, (512, 512)),
        image(5, 12, 0, (1024, 1024)),
    ];
    assert_eq!(
        ids(&pick(Kind::Square, &images)),
        ["5.png", "4.png", "3.png", "2.png", "1.png"]
    );
}

#[test]
fn only_the_sizes_of_the_kind_are_taken() {
    let images = [
        image(1, 1, 0, (600, 900)),
        image(2, 1, 0, (660, 930)),
        image(3, 9, 0, (512, 512)),
        image(4, 9, 0, (920, 430)),
        image(5, 9, 0, (0, 0)),
    ];
    assert_eq!(ids(&pick(Kind::Cover, &images)), ["1.png", "2.png"]);
    assert_eq!(ids(&pick(Kind::Square, &images)), ["3.png"]);
    assert_eq!(pick(Kind::Hero, &images), []);
    assert_eq!(ids(&pick(Kind::Logo, &images)).len(), 5);
}

#[test]
fn nothing_unsafe_a_joke_flashing_moving_or_insecure() {
    let safe = image(1, 0, 0, (512, 512));
    let mut nsfw = image(2, 9, 0, (512, 512));
    nsfw.nsfw = true;
    let mut humor = image(3, 9, 0, (512, 512));
    humor.humor = true;
    let mut epilepsy = image(4, 9, 0, (512, 512));
    epilepsy.epilepsy = true;
    let mut gif = image(5, 9, 0, (512, 512));
    gif.mime = "image/gif".to_owned();
    let mut http = image(6, 9, 0, (512, 512));
    http.url = "http://cdn.example/6.png".to_owned();
    let mut spaced = image(7, 9, 0, (512, 512));
    spaced.url = "https://cdn.example/7 .png".to_owned();
    let mut long = image(8, 9, 0, (512, 512));
    long.url = format!("https://cdn.example/{}.png", "a".repeat(URL_MAX));
    let images = [safe, nsfw, humor, epilepsy, gif, http, spaced, long];
    assert_eq!(ids(&pick(Kind::Square, &images)), ["1.png"]);
}

#[test]
fn a_picture_without_a_type_is_judged_by_its_name() {
    let mut jpeg = image(1, 0, 0, (512, 512));
    jpeg.mime.clear();
    jpeg.url = "https://cdn.example/1.JPG".to_owned();
    let mut gif = image(2, 0, 0, (512, 512));
    gif.mime.clear();
    gif.url = "https://cdn.example/2.gif".to_owned();
    assert_eq!(ids(&pick(Kind::Square, &[jpeg, gif])), ["1.JPG"]);
}

#[test]
fn the_same_address_twice_is_one_candidate() {
    let mut twin = image(2, 0, 0, (512, 512));
    twin.url = "https://cdn.example/1.png".to_owned();
    let picked = pick(Kind::Square, &[image(1, 1, 0, (512, 512)), twin]);
    assert_eq!(ids(&picked), ["1.png"]);
}

#[test]
fn a_picture_taken_is_of_the_kind_asked_for() {
    for (kind, size) in [
        (Kind::Square, (512, 512)),
        (Kind::Cover, (600, 900)),
        (Kind::Hero, (1920, 620)),
        (Kind::Logo, (1, 1)),
    ] {
        let picked = pick(kind, &[image(1, 0, 0, size)]);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].kind, kind);
    }
}

#[test]
fn the_style_says_whether_a_grid_shows_the_title_and_a_hero_never_does() {
    assert!(shows_title("alternate"));
    assert!(shows_title("white_logo"));
    assert!(shows_title("material"));
    assert!(shows_title(""));
    assert!(!shows_title("no_logo"));
    assert!(!shows_title("blurred"));
    let mut grid = image(1, 0, 0, (512, 512));
    grid.style = "no_logo".to_owned();
    assert!(!pick(Kind::Square, &[grid])[0].titled);
    assert!(pick(Kind::Cover, &[image(2, 0, 0, (600, 900))])[0].titled);
    assert!(!pick(Kind::Hero, &[image(3, 0, 0, (1920, 620))])[0].titled);
}
