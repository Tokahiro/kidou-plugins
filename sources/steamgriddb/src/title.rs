//! Finding a game by its title, for a store whose ids SteamGridDB does not know: only a game of
//! exactly the same title is taken, since wrong art is worse than none.

use crate::api::MARKS;
use crate::json::Game;

/// Signs left out within a word, so that "Baldur's" and "Baldur’s", or "S.T.A.L.K.E.R." and
/// "STALKER", are the same.
const WITHIN: &[char] = &['\'', '\u{2018}', '\u{2019}', '`', '\u{00b4}', '.'];

/// `title` as titles are compared: letters in lower case and digits, words split at every other
/// sign and joined by single spaces. Trademark signs, apostrophes, full stops and combining
/// accents are left out.
pub fn normalized(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut apart = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            if apart && !out.is_empty() {
                out.push(' ');
            }
            apart = false;
            out.extend(c.to_lowercase());
        } else if !(WITHIN.contains(&c) || MARKS.contains(&c) || combining(c)) {
            apart = true;
        }
    }
    out
}

/// SteamGridDB's id of the game titled `title` among `found`: only a game whose name is the same
/// title once both are [`normalized`], a verified one before one that is not, else the first.
pub fn choose(title: &str, found: &[Game]) -> Option<u64> {
    let wanted = normalized(title);
    if wanted.is_empty() {
        return None;
    }
    let same = || found.iter().filter(|game| normalized(&game.name) == wanted);
    same()
        .find(|game| game.verified)
        .or_else(|| same().next())
        .map(|game| game.id)
}

/// A combining accent, written after the letter it marks.
fn combining(c: char) -> bool {
    ('\u{0300}'..='\u{036f}').contains(&c)
}

#[cfg(test)]
mod tests;
