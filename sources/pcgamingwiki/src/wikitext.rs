//! The templates of a PCGamingWiki page the facts are read from. A game's page holds each of
//! them once, as `{{Name` and lines of `|key = value`, and a value may hold templates and links
//! of its own, which hold `|` too.

/// The named values of the first template `name` on `page`, in their order; none where the page
/// has no such template.
pub fn template(page: &str, name: &str) -> Vec<(String, String)> {
    let Some(body) = body(page, name) else {
        return Vec::new();
    };
    (top_level(body).into_iter())
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((key.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect()
}

/// The first argument of each template `name` on `page`, such as the engines' rows
/// `{{Infobox game/row/engine|The Forge|Renderer}}`, trimmed; empty ones are left out.
pub fn first_arguments(page: &str, name: &str) -> Vec<String> {
    let opening = format!("{{{{{name}|");
    let mut found = Vec::new();
    let mut rest = page;
    while let Some(start) = rest.find(&opening) {
        rest = rest.get(start.saturating_add(2)..).unwrap_or_default();
        let Some(body) = closed(rest) else {
            break;
        };
        let parts = top_level(body);
        if let Some(first) = parts.get(1).map(|part| part.trim())
            && !first.is_empty()
        {
            found.push(first.to_owned());
        }
    }
    found
}

/// Whether the page's availability rows, `{{Availability/row| Steam | …}}`, name the store
/// `shop`.
pub fn sold_at(page: &str, shop: &str) -> bool {
    (first_arguments(page, "Availability/row").iter()).any(|name| name.eq_ignore_ascii_case(shop))
}

/// What stands between `{{name` and its closing `}}`, where `name` ends at a line break, a `|`
/// or the `}}`: `{{Video game` is no `{{Video`.
fn body<'a>(page: &'a str, name: &str) -> Option<&'a str> {
    let opening = format!("{{{{{name}");
    let mut rest = page;
    loop {
        let start = rest.find(&opening)?;
        let after = rest.get(start.saturating_add(opening.len())..)?;
        let ends = (after.trim_start_matches(' ').chars().next())
            .is_none_or(|c| matches!(c, '|' | '\n' | '\r' | '}'));
        if ends {
            let inner = rest.get(start.saturating_add(2)..)?;
            return closed(inner);
        }
        rest = after;
    }
}

/// `text`, which starts just after a `{{`, up to the `}}` that closes it.
fn closed(text: &str) -> Option<&str> {
    let mut depth = 0_usize;
    let bytes = text.as_bytes();
    let mut at = 0_usize;
    while let Some(pair) = bytes.get(at..at.checked_add(2)?) {
        match pair {
            b"{{" => {
                depth = depth.checked_add(1)?;
                at = at.checked_add(2)?;
            }
            b"}}" if depth == 0 => return text.get(..at),
            b"}}" => {
                depth = depth.checked_sub(1)?;
                at = at.checked_add(2)?;
            }
            _ => at = at.checked_add(1)?,
        }
    }
    None
}

/// `body` split at each `|` that no inner template or link holds.
fn top_level(body: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0_usize;
    let bytes = body.as_bytes();
    let mut at = 0_usize;
    while let Some(&byte) = bytes.get(at) {
        let next = bytes.get(at.saturating_add(1)).copied();
        match (byte, next) {
            (b'{', Some(b'{')) | (b'[', Some(b'[')) => {
                depth = depth.saturating_add(1);
                at = at.saturating_add(2);
            }
            (b'}', Some(b'}')) | (b']', Some(b']')) => {
                depth = depth.saturating_sub(1);
                at = at.saturating_add(2);
            }
            (b'|', _) if depth == 0 => {
                parts.push(body.get(start..at).unwrap_or_default());
                at = at.saturating_add(1);
                start = at;
            }
            _ => at = at.saturating_add(1),
        }
    }
    parts.push(body.get(start..).unwrap_or_default());
    parts
}

#[cfg(test)]
mod tests;
