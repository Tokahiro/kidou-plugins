use kidou_layout::{CONTRACT, PARTS, VERSION};

/// The names the contract's files export, by the version that brought them. A new name takes a
/// new row with the next minor version, and [`VERSION`] and Kidō's `LAYOUT_CONTRACT` follow it.
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

/// The names `text` exports: each `export component`, `struct`, `enum` or `global`, and each
/// name of an `export { … }` list.
fn exported(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("export ") else {
            continue;
        };
        if let Some(list) = rest.trim_start().strip_prefix('{') {
            let (list, _) = list
                .split_once('}')
                .unwrap_or_else(|| panic!("`{line}`: an export list this test reads on one line"));
            for name in list.split(',').map(str::trim) {
                assert!(
                    !name.contains(' '),
                    "`{line}`: an export under another name, which this test does not read"
                );
                names.push(name);
            }
        } else {
            let name = rest
                .split_whitespace()
                .nth(1)
                .unwrap_or_else(|| panic!("`{line}`: an export with no name"));
            names.push(name);
        }
    }
    names
}

#[test]
fn the_contracts_exported_names_are_those_its_versions_brought() {
    let mut exported = [exported(CONTRACT.1), exported(PARTS.1)].concat();
    exported.sort_unstable();
    let mut brought: Vec<&str> = BROUGHT
        .iter()
        .flat_map(|(_, names)| names.iter().copied())
        .collect();
    brought.sort_unstable();
    assert_eq!(
        exported, brought,
        "a name the contract exports more is a new minor version: give it a row in BROUGHT"
    );
}

#[test]
fn the_contracts_version_is_the_latest_that_brought_a_name() {
    let (latest, _) = BROUGHT.last().expect("a version");
    assert_eq!(*latest, VERSION);
}
