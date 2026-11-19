use std::collections::BTreeSet;
use std::path::Path;

use kidou_layout::VERSION;

/// The names the contract's files export, by the version that brought them. A new name takes a
/// new row with the next minor version, never a place in a row already released, and
/// [`VERSION`] and Kidō's `LAYOUT_CONTRACT` follow it.
const BROUGHT: &[(&str, &[&str])] = &[
    (
        "3.0.0",
        &[
            "Cover",
            "CoverShadow",
            "EmptyCard",
            "Facts",
            "Glass",
            "GlassKind",
            "HeroTitle",
            "LayoutCardSize",
            "LayoutEmptyWords",
            "LayoutFact",
            "LayoutHero",
            "LayoutPlace",
            "LayoutTheme",
            "LayoutTile",
            "LayoutTitleLength",
            "LayoutZone",
            "Ring",
            "RingStyle",
            "RowCover",
        ],
    ),
    ("3.1.0", &["Kicker", "Shoulder"]),
];

/// The text of each file in the crate's `contract` folder: Home serves every one of them to a
/// layout by its name.
fn contract_files() -> Vec<String> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("contract");
    std::fs::read_dir(folder)
        .expect("the contract's folder")
        .map(|entry| std::fs::read_to_string(entry.expect("an entry").path()).expect("its text"))
        .collect()
}

/// The names `text` exports: each `export component`, `struct`, `enum` or `global`, and each
/// name of an `export { … }` list.
fn exported(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("export") else {
            continue;
        };
        if !rest.starts_with(|next: char| next.is_whitespace() || next == '{') {
            continue;
        }
        let found = if let Some(list) = rest.trim_start().strip_prefix('{') {
            let (list, _) = list
                .split_once('}')
                .unwrap_or_else(|| panic!("`{line}`: this test reads an export list on one line"));
            list.split(',').map(str::trim).collect()
        } else {
            vec![rest.split_whitespace().nth(1).unwrap_or_default()]
        };
        for name in found {
            assert!(
                !name.is_empty()
                    && name
                        .chars()
                        .all(|letter| letter.is_alphanumeric() || letter == '-' || letter == '_'),
                "`{line}`: this test reads `export <kind> Name` and `export {{ Name, Name }}` only"
            );
            names.push(name);
        }
    }
    names
}

#[test]
fn the_contracts_exported_names_are_those_its_versions_brought() {
    let files = contract_files();
    let exported: BTreeSet<&str> = files.iter().flat_map(|text| exported(text)).collect();
    let brought: BTreeSet<&str> = BROUGHT
        .iter()
        .flat_map(|(_, names)| names.iter().copied())
        .collect();
    assert_eq!(
        exported, brought,
        "a name the contract exports more is a new minor version: give it a new row in BROUGHT"
    );
}

#[test]
fn each_version_that_brought_a_name_is_the_next_minor_one_up_to_the_contracts() {
    let versions: Vec<String> = (0..BROUGHT.len())
        .map(|minor| format!("3.{minor}.0"))
        .collect();
    let brought: Vec<&str> = BROUGHT.iter().map(|(version, _)| *version).collect();
    assert_eq!(brought, versions);
    assert_eq!(brought.last(), Some(&VERSION));
}
