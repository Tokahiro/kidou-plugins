//! The facts a player of a handheld asks about, read from a game's PCGamingWiki page and worded
//! as Kidō's game page words them, under the heading „PCGamingWiki“ Kidō gives the plugin's rows.
//! The last row credits the wiki as its licence asks: the licence, that the facts are shortened,
//! and the page they come from (<https://www.pcgamingwiki.com/wiki/PCGamingWiki:Copyrights>).

use crate::api;
use crate::wikitext::{first_arguments, template};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    German,
}

/// A row of facts, as `kidou:source`'s `fact`.
#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub value: String,
}

/// What the wiki says a feature is, in its words `true`, `false`, `limited`, `hackable` and
/// `always on`; any other word, such as `unknown` or `n/a`, or none, says nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Yes,
    No,
    Limited,
    Hackable,
    AlwaysOn,
}

impl State {
    fn of(fields: &[(String, String)], key: &str) -> Option<Self> {
        let (_, value) = fields.iter().find(|(known, _)| known == key)?;
        match value.to_ascii_lowercase().as_str() {
            "true" => Some(Self::Yes),
            "false" => Some(Self::No),
            "limited" => Some(Self::Limited),
            "hackable" => Some(Self::Hackable),
            "always on" => Some(Self::AlwaysOn),
            _ => None,
        }
    }

    fn word(self, language: Language) -> &'static str {
        match (self, language) {
            (Self::Yes, Language::English) => "Yes",
            (Self::Yes, Language::German) => "Ja",
            (Self::No, Language::English) => "No",
            (Self::No, Language::German) => "Nein",
            (Self::Limited, Language::English) => "Limited",
            (Self::Limited, Language::German) => "Eingeschränkt",
            (Self::Hackable, Language::English) => "With a fix",
            (Self::Hackable, Language::German) => "Mit Fix",
            (Self::AlwaysOn, Language::English) => "Always on",
            (Self::AlwaysOn, Language::German) => "Immer an",
        }
    }
}

/// The cloud services whose saves the wiki names, by their key in `{{Save game cloud syncing}}`.
const CLOUDS: [(&str, &str); 6] = [
    ("steam cloud", "Steam"),
    ("gog galaxy", "GOG"),
    ("epic games launcher", "Epic"),
    ("xbox cloud", "Xbox"),
    ("ubisoft connect", "Ubisoft"),
    ("origin", "EA"),
];

/// The most engines named in the row.
const ENGINES_MAX: usize = 2;

/// The facts of the page `title` with the wikitext `page`, in Kidō's order: controller, frame
/// rate, ultrawide, HDR, cloud saves, engine, then the credit. None, and no credit, where the
/// page tells none of them.
pub fn rows(title: &str, page: &str, language: Language) -> Vec<Row> {
    let input = template(page, "Input");
    let video = template(page, "Video");
    let clouds = template(page, "Save game cloud syncing");
    let de = language == Language::German;
    let mut rows = Vec::new();
    let mut add = |en: &str, german: &str, value: Option<String>| {
        if let Some(value) = value {
            let label = if de { german } else { en };
            rows.push(Row {
                label: label.to_owned(),
                value,
            });
        }
    };
    add("Controller", "Controller", controller(&input, language));
    add("Frame rate", "Bildrate", frame_rate(&video, language));
    let state = |key| State::of(&video, key).map(|state| state.word(language).to_owned());
    add("Ultrawide", "Ultrawide", state("ultrawidescreen"));
    add("HDR", "HDR", state("hdr"));
    add(
        "Cloud saves",
        "Cloud-Spielstände",
        cloud_saves(&clouds, language),
    );
    add("Engine", "Engine", engines(page));
    if !rows.is_empty() {
        let (label, shortened) = if de {
            ("Lizenz", "gekürzt")
        } else {
            ("License", "shortened")
        };
        rows.push(Row {
            label: label.to_owned(),
            value: format!("CC BY-NC-SA 3.0, {shortened}, {}", api::page(title)),
        });
    }
    rows
}

/// Whether the game takes a controller: all of it, part of it, or with a fix.
fn controller(input: &[(String, String)], language: Language) -> Option<String> {
    let support = State::of(input, "controller support")?;
    let full = State::of(input, "full controller");
    let word = match (support, full, language) {
        (State::Yes | State::AlwaysOn, Some(State::Yes), Language::English) => "Full",
        (State::Yes | State::AlwaysOn, Some(State::Yes), Language::German) => "Vollständig",
        (State::Yes | State::AlwaysOn | State::Limited, _, Language::English) => "Partial",
        (State::Yes | State::AlwaysOn | State::Limited, _, Language::German) => "Teilweise",
        (State::Hackable | State::No, _, _) => support.word(language),
    };
    Some(word.to_owned())
}

/// How fast the game may run: over 60 frames a second, up to 60, or under.
fn frame_rate(video: &[(String, String)], language: Language) -> Option<String> {
    let above = State::of(video, "120 fps");
    let sixty = State::of(video, "60 fps");
    let [over, over_fix, up_to, sixty_only, under, sixty_fix] = match language {
        Language::English => [
            "Over 60 fps",
            "Over 60 fps with a fix",
            "Up to 60 fps",
            "60 fps",
            "Under 60 fps",
            "60 fps with a fix",
        ],
        Language::German => [
            "Über 60 fps",
            "Über 60 fps mit Fix",
            "Bis 60 fps",
            "60 fps",
            "Unter 60 fps",
            "60 fps mit Fix",
        ],
    };
    let word = match (above, sixty) {
        (Some(State::Yes | State::Limited | State::AlwaysOn), _) => over,
        (Some(State::Hackable), _) => over_fix,
        (Some(State::No), Some(State::Yes | State::Limited | State::AlwaysOn)) => up_to,
        (None, Some(State::Yes | State::Limited | State::AlwaysOn)) => sixty_only,
        (_, Some(State::No)) => under,
        (_, Some(State::Hackable)) => sixty_fix,
        (_, None) => return None,
    };
    Some(word.to_owned())
}

/// The services that keep the game's saves; „No“ where the wiki names none but says no for one.
fn cloud_saves(clouds: &[(String, String)], language: Language) -> Option<String> {
    let states = CLOUDS.map(|(key, name)| (State::of(clouds, key), name));
    let names: Vec<&str> = (states.iter())
        .filter(|(state, _)| matches!(state, Some(State::Yes | State::AlwaysOn)))
        .map(|&(_, name)| name)
        .collect();
    if !names.is_empty() {
        return Some(names.join(", "));
    }
    (states.iter())
        .any(|(state, _)| *state == Some(State::No))
        .then(|| State::No.word(language).to_owned())
}

/// The game's engines, as the infobox names them; links and templates in a name are left as
/// their text.
fn engines(page: &str) -> Option<String> {
    let names: Vec<String> = (first_arguments(page, "Infobox game/row/engine").into_iter())
        .map(|name| plain(&name))
        .filter(|name| !name.is_empty())
        .take(ENGINES_MAX)
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// `text` without the brackets of wiki links, a link shown by its label.
fn plain(text: &str) -> String {
    let unbracketed = text.replace("[[", "").replace("]]", "");
    let shown = unbracketed.rsplit('|').next().unwrap_or_default();
    shown.trim().to_owned()
}

#[cfg(test)]
mod tests;
