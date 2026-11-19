use kidou_layout::{Files, Mismatch, Record, Refused, compile, load, record_path};

use crate::{folder, good};

/// A change to a layout's folder, named.
type Change = (&'static str, fn(&std::path::Path));

/// The sample's files, copied into a folder of the test's own.
fn copied(name: &str) -> std::path::PathBuf {
    let to = folder(name);
    for file in ["layout.slint", "mark.svg"] {
        std::fs::copy(good().join(file), to.join(file)).expect("the sample's file");
    }
    to
}

#[test]
fn a_record_matches_the_files_it_was_written_of_by_the_same_version() {
    let files = Files::read(&good()).expect("the sample reads");
    let record = Record::of(&files, "1.2.3");
    assert_eq!(Record::parse(&record.text()), Some(record.clone()));
    assert_eq!(record.matches(&files, "1.2.3"), Ok(()));
    assert_eq!(record.matches(&files, "1.2.4"), Err(Mismatch::Version));
    assert_eq!(
        record_path(std::path::Path::new("data"), "carousel"),
        std::path::Path::new("data/layouts/carousel.record")
    );
}

#[test]
fn a_file_changed_added_or_removed_after_the_record_is_a_mismatch() {
    let changes: [Change; 4] = [
        ("changed", |at| {
            std::fs::write(at.join("mark.svg"), "changed").expect("the change");
        }),
        ("added", |at| {
            std::fs::write(at.join("more.slint"), "").expect("the change");
        }),
        ("removed", |at| {
            std::fs::remove_file(at.join("mark.svg")).expect("the change");
        }),
        ("renamed", |at| {
            std::fs::rename(at.join("mark.svg"), at.join("mark2.png")).expect("the change");
        }),
    ];
    for (name, change) in changes {
        let at = copied(name);
        let before = Files::read(&at).expect("the copy reads");
        let record = Record::of(&before, "1");
        change(&at);
        let after = Files::read(&at).expect("the changed copy reads");
        assert_eq!(record.matches(&after, "1"), Err(Mismatch::Files), "{name}");
    }
}

#[test]
fn the_compiled_layout_comes_from_the_copy_even_once_the_folder_is_gone() {
    let at = copied("gone");
    let files = crate::drawn(Files::read(&at).expect("the copy reads"));
    std::fs::remove_dir_all(&at).expect("the folder goes");
    assert!(compile(&files).is_ok());
}

#[test]
fn reading_refuses_links_project_files_hidden_files_and_the_names_kidou_serves() {
    let refused: [Change; 5] = [
        ("project", |at| {
            std::fs::write(at.join("slint-project.json"), "{}").expect("the file");
        }),
        ("hidden", |at| {
            std::fs::write(at.join(".hidden"), "").expect("the file");
        }),
        ("contract", |at| {
            std::fs::write(at.join(kidou_layout::CONTRACT.0), "").expect("the file");
        }),
        ("parts", |at| {
            std::fs::write(at.join(kidou_layout::PARTS.0), "").expect("the file");
        }),
        ("link", |at| {
            #[cfg(unix)]
            std::os::unix::fs::symlink("/etc", at.join("etc")).expect("the link");
        }),
    ];
    for (name, add) in refused {
        let at = copied(name);
        add(&at);
        assert!(Files::read(&at).is_err(), "{name}");
    }
}

#[test]
fn reading_holds_a_layout_to_its_caps() {
    let big = vec![0; usize::try_from(kidou_layout::MAX_FILE).expect("a size") + 1];
    assert!(Files::of([("big.png", big.as_slice())]).is_err());
    let names: Vec<String> = (0..=kidou_layout::MAX_FILES)
        .map(|n| format!("f{n}"))
        .collect();
    assert!(Files::of(names.iter().map(|name| (name.as_str(), b"".as_slice()))).is_err());
}

#[test]
fn packages_built_in_memory_keep_the_folder_depth_of_a_read() {
    assert!(Files::of([("a/b/c/x.slint", b"".as_slice())]).is_ok());
    assert!(Files::of([("a/b/c/d/x.slint", b"".as_slice())]).is_err());
}

/// The sample installed as `kidouplug` installs it, in a plugin folder of the test's own: its
/// layout, and the PNG drawn of its SVG.
fn installed(name: &str) -> std::path::PathBuf {
    let plugin = folder(name);
    let layout = plugin.join(kidou_layout::FOLDER);
    std::fs::create_dir_all(&layout).expect("the layout's folder");
    let files = crate::drawn(Files::read(&good()).expect("the sample reads"));
    for file in files.names() {
        std::fs::write(layout.join(file), files.get(file).unwrap_or_default()).expect("a file");
    }
    let drawn = plugin.join(kidou_layout::DRAWN);
    std::fs::create_dir_all(&drawn).expect("the drawn folder");
    for (file, png) in files.drawn() {
        std::fs::write(drawn.join(file), png).expect("a drawn PNG");
    }
    plugin
}

#[test]
fn an_installed_layout_reads_its_drawn_pngs_and_its_record_covers_them() {
    let plugin = installed("installed");
    let files = Files::installed(&plugin).expect("the installed layout reads");
    assert!(compile(&files).is_ok());
    let record = Record::of(&files, "1");
    let png = plugin.join(kidou_layout::DRAWN).join("mark.svg.png");
    let mut bytes = std::fs::read(&png).expect("the drawn PNG");
    bytes.push(0);
    std::fs::write(&png, bytes).expect("the change");
    let changed = Files::installed(&plugin).expect("the changed layout reads");
    assert_eq!(record.matches(&changed, "1"), Err(Mismatch::Files));
}

#[test]
fn an_installed_layout_has_one_drawn_png_for_each_svg_and_no_other() {
    let changes: [Change; 2] = [
        ("missing", |plugin| {
            std::fs::remove_file(plugin.join("drawn/mark.svg.png")).expect("the change");
        }),
        ("stray", |plugin| {
            std::fs::write(plugin.join("drawn/other.svg.png"), "").expect("the change");
        }),
    ];
    for (name, change) in changes {
        let plugin = installed(name);
        change(&plugin);
        assert!(Files::installed(&plugin).is_err(), "{name}");
    }
}

/// A case of Home's load: its name, the record's version, a line added to the layout after the
/// record, and what Home gets, `None` for the compiled layout.
type Load = (
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    Option<Refused>,
);

#[test]
fn home_loads_a_layout_only_while_its_record_matches_its_files() {
    let cases: [Load; 4] = [
        ("load-matches", Some("1.2.3"), None, None),
        ("load-no-record", None, None, Some(Refused::Off)),
        (
            "load-other-version",
            Some("1.2.2"),
            None,
            Some(Refused::Unchecked),
        ),
        (
            "load-changed",
            Some("1.2.3"),
            Some("// changed\n"),
            Some(Refused::Changed),
        ),
    ];
    for (case, version, added, refused) in cases {
        let plugin = installed(case);
        let layout = plugin.join(kidou_layout::FOLDER);
        let record = plugin.join("layout.record");
        if let Some(version) = version {
            let files = Files::installed(&plugin).expect("the sample reads");
            std::fs::write(&record, Record::of(&files, version).text()).expect("the record");
        }
        if let Some(added) = added {
            let root = layout.join(kidou_layout::ROOT);
            let mut text = std::fs::read_to_string(&root).expect("the layout");
            text.push_str(added);
            std::fs::write(&root, text).expect("the layout");
        }

        let loaded = load(&plugin, &record, "1.2.3");

        assert_eq!(loaded.err(), refused, "{case}");
    }
}

#[test]
fn a_layout_whose_folder_is_gone_is_broken() {
    let plugin = folder("load-gone");
    let record = plugin.join("layout.record");
    std::fs::write(
        &record,
        Record::of(&Files::read(&good()).unwrap(), "1").text(),
    )
    .unwrap();
    assert!(matches!(
        load(&plugin, &record, "1"),
        Err(Refused::Broken(_))
    ));
}
