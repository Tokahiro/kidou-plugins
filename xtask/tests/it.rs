//! The list, the packages and the sources of a repository made for the test: what `pack` writes
//! and what `check` finds.

use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use xtask::{MAX_ENTRIES, PACKAGES, check, layouts, pack_new, sha256, sources, unlisted};

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
    format!(
        "id = \"lamp\"\nname = \"Lampe\"\nversion = \"{version}\"\ncategory = \"layout\"\napi = \"3.1.0\"\n"
    )
}

/// A repository named `name` with the plugin `lamp` 1.0.0 as a source, listed and not yet packed.
fn repository(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("layouts/lamp/layout")).expect("the source's folder");
    fs::create_dir_all(root.join("devices")).expect("the devices' folder");
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
        "api = \"3.1.0\"".to_owned(),
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
    fs::create_dir_all(root.join("layouts/lamp/layout/bare/inner")).expect("folders of no file");
    fs::create_dir_all(root.join("layouts/lamp/layout/icons/bare")).expect("a folder of a file");
    fs::write(root.join("layouts/lamp/layout/icons/moon.svg"), "<svg/>").expect("a file below");
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
            "layout/icons",
            "layout/icons/moon.svg",
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
            "not-listed-does-not-pack",
            |root| {
                fs::create_dir_all(root.join("layouts/moth")).unwrap();
                let manifest = manifest("1.0.0").replace("lamp", "moth");
                fs::write(root.join("layouts/moth/plugin.toml"), manifest).unwrap();
            },
            "moth/layout: ",
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
fn the_lists_api_is_held_to_the_packages_manifest() {
    let api = "api = \"3.1.0\"\n";
    let names = "which the manifest in packages/lamp-1.0.0.kidouplug names";
    let gone = "lamp: no `api`; the manifest in packages/lamp-1.0.0.kidouplug names 3.1.0";
    for (name, listed, wrong) in [
        (
            "api-differs",
            "api = \"3.0.0\"\n",
            format!("lamp: `api` is not 3.1.0, {names}"),
        ),
        ("api-gone", "", gone.to_owned()),
    ] {
        let root = packed(name);
        let list = fs::read_to_string(root.join("index.toml")).expect("the list");
        assert!(list.contains(api), "{list}");
        fs::write(root.join("index.toml"), list.replace(api, listed)).expect("the list");
        assert_eq!(check(&root).expect("a check"), [wrong], "{name}");
    }
}

#[test]
fn a_source_whose_version_has_no_package_yet_passes_the_check() {
    let root = packed("pending");
    fs::write(root.join("layouts/lamp/plugin.toml"), manifest("1.1.0")).expect("the manifest");
    assert_eq!(check(&root).expect("a check"), [""; 0]);
}

#[test]
fn a_source_the_list_does_not_name_is_checked_and_not_packed() {
    let root = packed("unlisted");
    fs::create_dir_all(root.join("layouts/moth/layout")).expect("the source's folder");
    let manifest = manifest("1.0.0").replace("lamp", "moth");
    fs::write(root.join("layouts/moth/plugin.toml"), manifest).expect("the manifest");
    let list = fs::read(root.join("index.toml")).expect("the list");
    assert_eq!(check(&root).expect("a check"), [""; 0]);
    assert_eq!(unlisted(&root).expect("the sources"), ["moth"]);
    assert_eq!(pack_new(&root).expect("a pack"), [""; 0]);
    assert!(!root.join("packages/moth-1.0.0.kidouplug").exists());
    assert_eq!(fs::read(root.join("index.toml")).expect("the list"), list);
}

/// A repository named `name` whose one source is the device plugin `lantern` 1.0.0, a crate of
/// its own workspace with no dependency, listed and not yet packed.
fn code_repository(name: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&root);
    let plugin = root.join("devices/lantern");
    fs::create_dir_all(plugin.join("src")).expect("the source's folder");
    fs::create_dir_all(root.join("layouts")).expect("the layouts' folder");
    fs::create_dir_all(root.join("packages")).expect("the packages' folder");
    for (file, text) in [
        (
            "index.toml",
            "[[plugin]]\nid = \"lantern\"\nname = \"Laterne\"\ncategory = \"device\"\n",
        ),
        (
            "Cargo.toml",
            "[workspace]\nresolver = \"3\"\nmembers = [\"devices/*\"]\n",
        ),
        (
            "Cargo.lock",
            "version = 4\n\n[[package]]\nname = \"plugin-lantern\"\nversion = \"0.0.0\"\n",
        ),
        (
            "devices/lantern/plugin.toml",
            "id = \"lantern\"\nversion = \"1.0.0\"\ncategory = \"device\"\napi = \"1.0.0\"\n",
        ),
        (
            "devices/lantern/Cargo.toml",
            "[package]\nname = \"plugin-lantern\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
             [lib]\nname = \"wick\"\ncrate-type = [\"cdylib\"]\n",
        ),
        (
            "devices/lantern/src/lib.rs",
            "#[unsafe(no_mangle)]\npub extern \"C\" fn lit(wicks: u32) -> u32 {\n    \
             wicks.checked_add(1).expect(\"a wick more\")\n}\n",
        ),
    ] {
        fs::write(root.join(file), text).expect("a file of the repository");
    }
    root
}

#[test]
fn a_plugin_with_code_is_built_and_packed_to_the_same_bytes_in_any_folder() {
    let root = code_repository("code");
    assert_eq!(check(&root).expect("a check"), [""; 0]);
    assert_eq!(pack_new(&root).expect("a pack"), ["lantern 1.0.0"]);
    let package = fs::read(root.join("packages/lantern-1.0.0.kidouplug")).expect("the package");
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(&package[..]));
    let files: Vec<(String, Vec<u8>)> = (archive.entries().expect("the entries"))
        .map(|entry| {
            let mut entry = entry.expect("an entry");
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).expect("its bytes");
            (entry.path().expect("a name").display().to_string(), bytes)
        })
        .collect();
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["plugin.toml", "plugin.wasm"]);
    assert!(files[1].1.starts_with(b"\0asm"), "no WebAssembly");
    assert_eq!(check(&root).expect("a check"), [""; 0]);

    let elsewhere = code_repository("code-in-another-folder");
    let source = sources(&elsewhere).expect("the sources").remove(0);
    assert_eq!(source.pack().expect("a pack"), package);

    fs::write(
        root.join("devices/lantern/src/lib.rs"),
        "#[unsafe(no_mangle)]\npub extern \"C\" fn lit(wicks: u32) -> u32 {\n    wicks\n}\n",
    )
    .expect("other code");
    let wrong = check(&root).expect("a check");
    assert!(
        wrong[0].starts_with("lantern: the source differs from packages/lantern-1.0.0.kidouplug"),
        "{wrong:?}"
    );
}

#[test]
fn a_plugin_whose_code_does_not_build_does_not_pack() {
    let root = code_repository("code-broken");
    fs::write(root.join("devices/lantern/src/lib.rs"), "fn lit(").expect("broken code");
    let list = fs::read_to_string(root.join("index.toml")).expect("the list");
    fs::write(root.join("index.toml"), list.replace("device", "events")).expect("the list");
    let wrong = check(&root).expect("a check");
    assert_eq!(
        wrong,
        [
            "lantern: the list names another category",
            "lantern: cargo could not build `plugin-lantern`"
        ]
    );
    pack_new(&root).expect_err("a refusal");
}

#[test]
fn an_id_of_two_categories_is_refused() {
    let root = code_repository("id-twice");
    fs::create_dir_all(root.join("layouts/lantern")).expect("the second source's folder");
    let manifest = manifest("1.0.0").replace("lamp", "lantern");
    fs::write(root.join("layouts/lantern/plugin.toml"), manifest).expect("the manifest");
    let refused = check(&root).expect_err("a refusal");
    assert!(refused.ends_with("a second plugin `lantern`"), "{refused}");
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
            "id = \"lamp\"\nversion = \"1.0\"\ncategory = \"layout\"\napi = \"3.1.0\"\n",
            "`1.0` is not three numbers",
        ),
        (
            "id = \"lamp\"\nversion = \"+1.0.0\"\ncategory = \"layout\"\napi = \"3.1.0\"\n",
            "`+1.0.0` is not three numbers",
        ),
        (
            "id = \"lamp\"\nversion = \"01.0.0\"\ncategory = \"layout\"\napi = \"3.1.0\"\n",
            "`01.0.0` is not three numbers",
        ),
        ("id = \"lamp\"\ncategory = \"layout\"\n", "no `version`"),
        (
            "id = \"lamp\"\nversion = \"1.0.0\"\ncategory = \"layout\"\n",
            "no `api`",
        ),
        (
            "id = \"lamp\"\nversion = \"1.0.0\"\ncategory = \"layout\"\napi = \"3.1\"\n",
            "`3.1` is not three numbers",
        ),
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

#[test]
fn layouts_names_each_layout_kidou_would_refuse() {
    let root = repository("layouts");
    let drawer = Path::new(env!("CARGO_BIN_EXE_xtask"));
    let wrong = layouts(&root, drawer).expect("a check");
    assert!(!wrong.is_empty(), "a file with no `Layout` passed");
    assert!(
        wrong.iter().all(|line| line.starts_with("lamp: ")),
        "{wrong:?}"
    );
    let good = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/layout/tests/it/good");
    for file in ["layout.slint", "mark.svg"] {
        fs::copy(good.join(file), root.join("layouts/lamp/layout").join(file)).expect("a copy");
    }
    assert_eq!(layouts(&root, drawer).expect("a check"), [""; 0]);
    // The SVG is drawn by the drawer, in a process of its own, and by nothing else.
    let wrong = layouts(&root, Path::new("no-such-drawer")).expect("a check");
    assert_eq!(wrong.len(), 1, "{wrong:?}");
    assert!(wrong[0].contains("mark.svg: no-such-drawer: "), "{wrong:?}");
}

#[test]
fn the_interfaces_parse() {
    let wit = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wit");
    let mut resolve = wit_parser::Resolve::default();
    // The folder itself is `kidou:store` with `deps/host`; a package comes after those it uses.
    let folders = ["events", "device", "source", "tiles"];
    resolve.push_dir(&wit).expect("store.wit and deps/host");
    for folder in folders {
        resolve.push_dir(wit.join(folder)).expect(folder);
    }
    let mut found: Vec<String> = fs::read_dir(&wit)
        .expect("the folder")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .into_string()
                .expect("a name")
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        ["deps", "device", "events", "source", "store.wit", "tiles"]
    );
}
