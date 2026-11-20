//! The time to beat as rows of facts, worded as Kidō's game page words it, under the heading
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

/// A row for each time `beat` tells, in its order: the main story, with extras, everything.
/// Each is in whole hours, rounded to the nearest, halves up, and at least 1.
pub fn rows(beat: TimeToBeat, language: Language) -> Vec<Row> {
    let labels = match language {
        Language::English => ["Main story", "With extras", "Everything"],
        Language::German => ["Hauptstory", "Mit Extras", "Komplett"],
    };
    let times = [beat.main, beat.extras, beat.complete];
    (labels.into_iter().zip(times))
        .filter_map(|(label, minutes)| {
            let hours = (minutes?.saturating_add(30) / 60).max(1);
            Some(Row {
                label: label.to_owned(),
                value: match language {
                    Language::English => format!("≈ {hours} h"),
                    Language::German => format!("≈ {hours} Std."),
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Language, Row, TimeToBeat, rows};

    fn row(label: &str, value: &str) -> Row {
        Row {
            label: label.to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn each_time_told_is_a_row_in_whole_hours() {
        let beat = TimeToBeat {
            main: Some(480),
            extras: Some(629),
            complete: Some(1230),
        };
        assert_eq!(
            rows(beat, Language::English),
            [
                row("Main story", "≈ 8 h"),
                row("With extras", "≈ 10 h"),
                row("Everything", "≈ 21 h"),
            ]
        );
        assert_eq!(
            rows(beat, Language::German),
            [
                row("Hauptstory", "≈ 8 Std."),
                row("Mit Extras", "≈ 10 Std."),
                row("Komplett", "≈ 21 Std."),
            ]
        );
    }

    #[test]
    fn only_the_times_told_and_at_least_an_hour() {
        let beat = TimeToBeat {
            main: None,
            extras: Some(5),
            complete: None,
        };
        assert_eq!(rows(beat, Language::English), [row("With extras", "≈ 1 h")]);
        assert_eq!(rows(TimeToBeat::default(), Language::German), Vec::new());
    }
}
