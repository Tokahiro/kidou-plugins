//! The list, the packages and the sources of a repository made for the test: what `pack` writes
//! and what `check` finds.

use std::fs;
use std::path::{Path, PathBuf};

use xtask::{MAX_ENTRIES, PACKAGES, check, pack_new, sha256, sources};

const LIST: &str = r#"# The list.

[[plugin]]
id = "lamp"
name = "Lampe"
category = "layout"
about = "Home layout: a lamp"

[[pack]]
id = "all"
name = "Alle"
plugins = ["lamp"]
"#;

fn manifest(version: &str) -> String {
    format!("id = \"lamp\"\nname = \"Lampe\"\nversion = \"{version}\"\ncategory = \"layout\"\n")
}

/// A repository named `name` with the plugin `lamp` 1.0.0 as a source, listed and not yet packed.
fn repository(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("layouts/lamp/layout")).expect("the source's folder");
    fs::create_dir_all(root.join("packages")).expect("the packages' folder");
    fs::write(root.join("index.toml"), LIST).expect("the list");
    fs::write(root.join("layouts/lamp/plugin.toml"), manifest("1.0.0")).expect("the manifest");
    fs::write(
        root.join("layouts/lamp/layout/layout.slint"),
        "// A lamp.\n",
    )
    .expect("the layout");
    root
}

/// A repository whose plugin `lamp` 1.0.0 is packed and listed.
fn packed(name: &str) -> PathBuf {
    let root = repository(name);
    pack_new(&root).expect("a pack");
    root
}

#[test]
fn a_new_version_is_packed_and_listed_once() {
    let root = repository("new-version");
    assert_eq!(pack_new(&root).expect("a pack"), ["lamp 1.0.0"]);
    let package = fs::read(root.join("packages/lamp-1.0.0.kidouplug")).expect("the package");
    let source = sources(&root).expect("the sources").remove(0);
    assert_eq!(package, source.pack().expect("the same pack"));
    let list = fs::read_to_string(root.join("index.toml")).expect("the list");
    for line in [
        "# The list.".to_owned(),
        "version = \"1.0.0\"".to_owned(),
        format!("package = \"{PACKAGES}/lamp-1.0.0.kidouplug\""),
        format!("sha256 = \"{}\"", sha256(&package)),
        "about = \"Home layout: a lamp\"".to_owned(),
    ] {
        assert!(list.contains(&line), "{line} is not in:\n{list}");
    }
    assert_eq!(check(&root).expect("a check"), [""; 0]);
    assert_eq!(pack_new(&root).expect("a second pack"), [""; 0]);
}

#[test]
fn a_package_holds_the_manifest_and_then_the_layout_each_folder_before_its_files() {
    let root = repository("order");
    fs::write(root.join("layouts/lamp/layout/art.svg"), "<svg/>").expect("a second file");
    fs::write(root.join("layouts/lamp/preview.png"), "a picture").expect("a file beside");
    let source = sources(&root).expect("the sources").remove(0);
    let package = source.pack().expect("a pack");
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(&package[..]));
    let names: Vec<String> = (archive.entries().expect("the entries"))
        .map(|entry| {
            let entry = entry.expect("an entry");
            assert_eq!(entry.header().mtime().expect("a time"), 0);
            entry.path().expect("a name").display().to_string()
        })
        .collect();
    assert_eq!(
        names,
        [
            "plugin.toml",
            "layout",
            "layout/art.svg",
            "layout/layout.slint"
        ]
    );
}

#[test]
fn check_names_what_differs() {
    type Change = fn(&Path);
    let cases: Vec<(&str, Change, &str)> = vec![
        (
            "source-changed",
            |root| fs::write(root.join("layouts/lamp/layout/layout.slint"), "// Lit.\n").unwrap(),
            "lamp: the source differs from packages/lamp-1.0.0.kidouplug",
        ),
        (
            "package-changed",
            |root| fs::write(root.join("packages/lamp-1.0.0.kidouplug"), "other").unwrap(),
            "lamp: `sha256` is not that of packages/lamp-1.0.0.kidouplug",
        ),
        (
            "package-gone",
            |root| fs::remove_file(root.join("packages/lamp-1.0.0.kidouplug")).unwrap(),
            "lamp: packages/lamp-1.0.0.kidouplug: ",
        ),
        (
            "not-listed",
            |root| {
                fs::create_dir_all(root.join("layouts/moth/layout")).unwrap();
                let manifest = manifest("1.0.0").replace("lamp", "moth");
                fs::write(root.join("layouts/moth/plugin.toml"), manifest).unwrap();
            },
            "moth: the list has no [[plugin]] of this id",
        ),
        (
            "pack-names-unknown",
            |root| {
                let list = fs::read_to_string(root.join("index.toml")).unwrap();
                let list = list.replace("[\"lamp\"]", "[\"lamp\", \"moth\"]");
                fs::write(root.join("index.toml"), list).unwrap();
            },
            "pack all: the list offers no plugin `moth`",
        ),
        (
            "list-behind",
            |root| {
                let list = fs::read_to_string(root.join("index.toml")).unwrap();
                fs::write(root.join("layouts/lamp/plugin.toml"), manifest("1.1.0")).unwrap();
                pack_new(root).unwrap();
                fs::write(root.join("index.toml"), list).unwrap();
            },
            "lamp: the list names 1.0.0, the source 1.1.0",
        ),
        (
            "version-earlier",
            |root| fs::write(root.join("layouts/lamp/plugin.toml"), manifest("0.9.0")).unwrap(),
            "lamp: the new version 0.9.0 is not later than the listed 1.0.0",
        ),
        (
            "device-name",
            |root| {
                fs::write(root.join("layouts/lamp/plugin.toml"), manifest("1.1.0")).unwrap();
                fs::write(root.join("layouts/lamp/layout/aux.svg"), "<svg/>").unwrap();
            },
            "\"aux.svg\" is no name a package may hold",
        ),
        (
            "too-many-files",
            |root| {
                fs::write(root.join("layouts/lamp/plugin.toml"), manifest("1.1.0")).unwrap();
                for at in 0..MAX_ENTRIES {
                    fs::write(root.join(format!("layouts/lamp/layout/{at}.svg")), "").unwrap();
                }
            },
            "a package holds at most 256 files and folders",
        ),
        (
            "listed-without-source",
            |root| {
                let list = fs::read_to_string(root.join("index.toml")).unwrap();
                let list = format!("{list}\n[[plugin]]\nid = \"moth\"\ncategory = \"store\"\n");
                fs::write(root.join("index.toml"), list).unwrap();
            },
            "moth: no `version`, and no source to pack",
        ),
        #[cfg(unix)]
        (
            "layout-is-a-link",
            |root| {
                fs::write(root.join("layouts/lamp/plugin.toml"), manifest("1.1.0")).unwrap();
                fs::remove_dir_all(root.join("layouts/lamp/layout")).unwrap();
                std::os::unix::fs::symlink(root.join("packages"), root.join("layouts/lamp/layout"))
                    .unwrap();
            },
            "layout: no folder",
        ),
    ];
    for (name, change, line) in cases {
        let root = packed(name);
        change(&root);
        let wrong = check(&root).expect("a check");
        assert!(
            wrong.iter().any(|wrong| wrong.contains(line)),
            "{name}: {wrong:?}"
        );
    }
    assert_eq!(check(&packed("nothing")).expect("a check"), [""; 0]);
}

#[test]
fn a_source_whose_version_has_no_package_yet_passes_the_check() {
    let root = packed("pending");
    fs::write(root.join("layouts/lamp/plugin.toml"), manifest("1.1.0")).expect("the manifest");
    assert_eq!(check(&root).expect("a check"), [""; 0]);
}

#[test]
fn a_manifest_that_does_not_fit_its_folder_is_refused() {
    let cases = [
        (
            "id = \"moth\"\nversion = \"1.0.0\"\ncategory = \"layout\"\n",
            "the folder is not named `moth`",
        ),
        (
            "id = \"lamp\"\nversion = \"1.0.0\"\ncategory = \"look\"\n",
            "no `layout` plugin",
        ),
        (
            "id = \"lamp\"\nversion = \"1.0\"\ncategory = \"layout\"\n",
            "`1.0` is not three numbers",
        ),
        (
            "id = \"lamp\"\nversion = \"+1.0.0\"\ncategory = \"layout\"\n",
            "`+1.0.0` is not three numbers",
        ),
        (
            "id = \"lamp\"\nversion = \"01.0.0\"\ncategory = \"layout\"\n",
            "`01.0.0` is not three numbers",
        ),
        ("id = \"lamp\"\ncategory = \"layout\"\n", "no `version`"),
    ];
    for (at, (manifest, why)) in cases.into_iter().enumerate() {
        let root = repository(&format!("manifest-{at}"));
        fs::write(root.join("layouts/lamp/plugin.toml"), manifest).expect("the manifest");
        let refused = check(&root).expect_err("a refusal");
        assert!(refused.ends_with(why), "{refused}");
    }
}

#[test]
fn an_id_that_kidos_list_would_drop_is_refused() {
    for id in [
        "my-lamp",
        "Lamp",
        "lamp.one",
        "a_lamp_of_seventeen",
        "1lamp",
    ] {
        let root = repository(&format!("id-{id}"));
        fs::rename(root.join("layouts/lamp"), root.join("layouts").join(id)).expect("the folder");
        let manifest = manifest("1.0.0").replace("id = \"lamp\"", &format!("id = \"{id}\""));
        fs::write(root.join("layouts").join(id).join("plugin.toml"), manifest).expect("a manifest");
        let refused = check(&root).expect_err("a refusal");
        assert!(refused.contains("is no id of Kidō's"), "{id}: {refused}");
    }
}
