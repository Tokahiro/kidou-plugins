//! The time to beat as a row of facts, worded as Kidō's game page words it, under the heading
//! „IGDB“ that Kidō gives the plugin's rows.

use crate::api::TimeToBeat;

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

/// The one time to beat Kidō's ADR-0024 shows: the main story's, else the one with the extras,
/// else everything's; none where `beat` tells no time. It is in whole hours, rounded to the
/// nearest, halves up, and at least 1.
pub fn row(beat: TimeToBeat, language: Language) -> Option<Row> {
    let labels = match language {
        Language::English => ["Main story", "With extras", "Everything"],
        Language::German => ["Hauptstory", "Mit Extras", "Komplett"],
    };
    let times = [beat.main, beat.extras, beat.complete];
    let (label, minutes) =
        (labels.into_iter().zip(times)).find_map(|(label, minutes)| Some((label, minutes?)))?;
    let hours = (minutes.saturating_add(30) / 60).max(1);
    Some(Row {
        label: label.to_owned(),
        value: match language {
            Language::English => format!("≈ {hours} h"),
            Language::German => format!("≈ {hours} Std."),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{Language, Row, TimeToBeat, row};

    fn told(label: &str, value: &str) -> Row {
        Row {
            label: label.to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn the_first_time_told_is_the_row_in_whole_hours() {
        let all = TimeToBeat {
            main: Some(480),
            extras: Some(629),
            complete: Some(1230),
        };
        assert_eq!(
            row(all, Language::English),
            Some(told("Main story", "≈ 8 h"))
        );
        assert_eq!(
            row(all, Language::German),
            Some(told("Hauptstory", "≈ 8 Std."))
        );
        let extras = TimeToBeat { main: None, ..all };
        assert_eq!(
            row(extras, Language::English),
            Some(told("With extras", "≈ 10 h"))
        );
        let complete = TimeToBeat {
            complete: Some(1230),
            ..TimeToBeat::default()
        };
        assert_eq!(
            row(complete, Language::German),
            Some(told("Komplett", "≈ 21 Std."))
        );
    }

    #[test]
    fn at_least_an_hour_and_none_without_a_time() {
        let short = TimeToBeat {
            extras: Some(5),
            ..TimeToBeat::default()
        };
        assert_eq!(
            row(short, Language::English),
            Some(told("With extras", "≈ 1 h"))
        );
        assert_eq!(row(TimeToBeat::default(), Language::German), None);
    }
}
