//! Kidō's plugin list held to the sources of this repository: `check` finds what differs, and
//! `pack` writes the package of each source whose version has none and lists it.
//!
//! The package rules here are those of `kidouplug pack`, and `layouts` runs the check Kidō runs
//! on a Home layout at the install. Whether a manifest is one Kidō takes is Kidō's check alone.
//!
//! A plugin with code is built for `wasm32-wasip2` before it is packed. The compiler, the
//! locked dependencies and the release profile are fixed in this repository, so the same source
//! gives the same component.

use std::fmt::Write as _;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use kidou_layout::Files;
use sha2::{Digest, Sha256};
use tar::{Archive, Builder, EntryType, Header};
use toml_edit::{DocumentMut, Table, value};

/// Where Kidō reads a package of this repository.
pub const PACKAGES: &str = "https://raw.githubusercontent.com/Tokahiro/kidou-plugins/main/packages";

/// Each category whose plugins have their source here: its name, the folder that holds them,
/// one folder per plugin named by its id, and what a plugin of it brings beside its manifest.
pub const SOURCES: [(&str, &str, Brings); 2] = [
    ("layout", "layouts", Brings::Folder(kidou_layout::FOLDER)),
    ("device", "devices", Brings::Code),
];

/// What a plugin's package holds beside its manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brings {
    /// A folder of this name, as the source holds it.
    Folder(&'static str),
    /// [`WASM`], the component built from the source's crate.
    Code,
}

/// The name of a plugin's component in its package.
pub const WASM: &str = "plugin.wasm";

/// The most bytes of a component Kidō reads.
pub const MAX_COMPONENT: usize = 64 << 20;

/// What a plugin with code is built for.
const TARGET: &str = "wasm32-wasip2";

/// The most files and folders Kidō unpacks from a package.
pub const MAX_ENTRIES: usize = 256;

/// The most bytes Kidō unpacks from a package.
pub const MAX_UNPACKED: usize = 128 << 20;

/// The most bytes of a manifest Kidō reads.
pub const MAX_MANIFEST: usize = 64 << 10;

/// The longest id Kidō's list takes.
pub const ID_MAX: usize = 16;

const LIST: &str = "index.toml";
const MANIFEST: &str = "plugin.toml";

/// A plugin whose source this repository holds.
#[derive(Debug)]
pub struct Source {
    pub folder: PathBuf,
    pub id: String,
    pub version: String,
    /// The version of its category's interface it is built for.
    pub api: String,
    pub category: &'static str,
    brings: Brings,
    root: PathBuf,
}

impl Source {
    /// The name of its version's package file.
    pub fn package(&self) -> String {
        format!("{}-{}.kidouplug", self.id, self.version)
    }

    /// Its package, as `kidouplug pack` writes it: a gzip'd tar of the manifest and what its
    /// category brings, a component built here or a folder, each file and folder by name and
    /// each folder before what it holds. The same source gives the same bytes. Refuses what
    /// Kidō would not unpack: a link, a name that does not fit, a component over
    /// [`MAX_COMPONENT`] bytes, more than [`MAX_ENTRIES`] entries or [`MAX_UNPACKED`] bytes.
    pub fn pack(&self) -> Result<Vec<u8>, String> {
        let manifest = self.folder.join(MANIFEST);
        let text = fs::read(&manifest).map_err(|why| format!("{}: {why}", manifest.display()))?;
        if text.len() > MAX_MANIFEST {
            return Err(format!(
                "{}: larger than {MAX_MANIFEST} bytes",
                manifest.display()
            ));
        }
        let mut files = vec![(MANIFEST.to_owned(), Some(text))];
        match self.brings {
            Brings::Code => files.push((WASM.to_owned(), Some(self.component()?))),
            Brings::Folder(brings) => {
                files.push((brings.to_owned(), None));
                walk(&self.folder, brings, &mut files)?;
            }
        }
        let bytes = files.iter().filter_map(|(_, bytes)| bytes.as_ref());
        if files.len() > MAX_ENTRIES || bytes.map(Vec::len).sum::<usize>() > MAX_UNPACKED {
            return Err(format!(
                "{}: a package holds at most {MAX_ENTRIES} files and folders, of at most \
                 {MAX_UNPACKED} bytes together",
                self.folder.display()
            ));
        }
        let mut tar = Builder::new(GzEncoder::new(Vec::new(), Compression::best()));
        for (name, bytes) in &files {
            let mut header = Header::new_gnu();
            header.set_mtime(0);
            let (kind, mode, data) = (bytes.as_deref())
                .map_or((EntryType::Directory, 0o755, &[][..]), |bytes| {
                    (EntryType::Regular, 0o644, bytes)
                });
            header.set_entry_type(kind);
            header.set_mode(mode);
            header.set_size(data.len() as u64);
            (tar.append_data(&mut header, name, data)).map_err(|why| format!("{name}: {why}"))?;
        }
        (tar.into_inner().and_then(GzEncoder::finish)).map_err(|why| why.to_string())
    }

    /// The component of its crate, built by cargo for [`TARGET`] with the release profile and
    /// the locked dependencies of the workspace, in a target folder of its own.
    fn component(&self) -> Result<Vec<u8>, String> {
        let at = self.folder.join("Cargo.toml");
        let manifest = read(&at)?;
        let word = |table: &str| manifest.get(table)?.get("name")?.as_str();
        let name = word("package").ok_or_else(|| format!("{}: no `package.name`", at.display()))?;
        let library = word("lib").unwrap_or(name).replace('-', "_");
        let target = self.root.join("target").join("code");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let built = Command::new(cargo)
            .current_dir(&self.root)
            .args(["build", "--locked", "--quiet", "--release"])
            .args(["--target", TARGET, "-p", name, "--target-dir"])
            .arg(&target)
            .stdout(Stdio::null())
            .status()
            .map_err(|why| format!("{}: cargo did not start: {why}", self.id))?;
        if !built.success() {
            return Err(format!("{}: cargo could not build `{name}`", self.id));
        }
        let file = (target.join(TARGET).join("release")).join(format!("{library}.wasm"));
        let bytes = fs::read(&file).map_err(|why| format!("{}: {why}", file.display()))?;
        if bytes.len() > MAX_COMPONENT {
            return Err(format!(
                "{}: larger than {MAX_COMPONENT} bytes",
                file.display()
            ));
        }
        Ok(bytes)
    }
}

/// Each plugin with its source under `root`, by category and id; refuses a manifest whose id is
/// not its folder's name, none Kidō's list takes or that of a plugin of another category, whose
/// category is not its folder's, or whose version or `api` is not three numbers.
pub fn sources(root: &Path) -> Result<Vec<Source>, String> {
    let mut found = Vec::new();
    for (category, folder, brings) in SOURCES {
        let all = root.join(folder);
        let unreadable = |why: std::io::Error| format!("{}: {why}", all.display());
        let mut folders = Vec::new();
        for entry in fs::read_dir(&all).map_err(unreadable)? {
            let entry = entry.map_err(unreadable)?;
            if entry.file_type().map_err(unreadable)?.is_dir() {
                folders.push(entry.path());
            }
        }
        folders.sort();
        for folder in folders {
            let at = folder.join(MANIFEST);
            let manifest = read(&at)?;
            let word = |key: &str| {
                (manifest.get(key).and_then(|item| item.as_str()))
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{}: no `{key}`", at.display()))
            };
            let (id, version) = (word("id")?, word("version")?);
            if folder.file_name().and_then(|name| name.to_str()) != Some(&id) {
                return Err(format!("{}: the folder is not named `{id}`", at.display()));
            }
            let rest =
                |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_';
            if !(id.starts_with(|first: char| first.is_ascii_lowercase())
                && id.bytes().all(rest)
                && id.len() <= ID_MAX
                && fit(&id))
            {
                return Err(format!(
                    "{}: `{id}` is no id of Kidō's: a lowercase letter, then lowercase letters, \
                     digits and `_`, at most {ID_MAX} in all",
                    at.display()
                ));
            }
            if word("category")? != category {
                return Err(format!("{}: no `{category}` plugin", at.display()));
            }
            if found.iter().any(|other: &Source| other.id == id) {
                return Err(format!("{}: a second plugin `{id}`", at.display()));
            }
            let api = word("api")?;
            for word in [&version, &api] {
                if numbers(word).is_none() {
                    return Err(format!("{}: `{word}` is not three numbers", at.display()));
                }
            }
            found.push(Source {
                folder,
                id,
                version,
                api,
                category,
                brings,
                root: root.to_owned(),
            });
        }
    }
    Ok(found)
}

/// The three numbers of the version `version`, each written in digits with no leading zero.
fn numbers(version: &str) -> Option<[u32; 3]> {
    let mut parts = version.split('.').map(|part| {
        let plain = part.bytes().all(|byte| byte.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'));
        part.parse::<u32>().ok().filter(|_| plain)
    });
    let numbers = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(numbers)
}

/// Adds the files and folders below `folder`'s `at`, a folder that is no link, to `files`, by
/// name, each folder before what it holds; a folder with no file below it is left out.
fn walk(folder: &Path, at: &str, files: &mut Vec<(String, Option<Vec<u8>>)>) -> Result<(), String> {
    let path = folder.join(at);
    let unreadable = |why: std::io::Error| format!("{}: {why}", path.display());
    if !fs::symlink_metadata(&path).map_err(unreadable)?.is_dir() {
        return Err(format!("{}: no folder", path.display()));
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(&path).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !fit(&name) {
            return Err(format!(
                "{}: {name:?} is no name a package may hold: only [A-Za-z0-9._-], not starting \
                 with a dot, and no device name of Windows",
                path.display()
            ));
        }
        names.push((name, entry.file_type().map_err(unreadable)?));
    }
    names.sort_by(|(one, _), (other, _)| one.cmp(other));
    for (name, kind) in names {
        let inner = format!("{at}/{name}");
        let file = folder.join(&inner);
        if kind.is_dir() {
            files.push((inner.clone(), None));
            let held = files.len();
            walk(folder, &inner, files)?;
            if files.len() == held {
                files.pop();
            }
        } else if kind.is_file() {
            let bytes = fs::read(&file).map_err(|why| format!("{}: {why}", file.display()))?;
            files.push((inner, Some(bytes)));
        } else {
            return Err(format!("{}: a link", file.display()));
        }
    }
    Ok(())
}

/// Whether Kidō unpacks a file or folder of this name: Windows opens a device for a device
/// name, with any extension.
fn fit(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default();
    let stem = stem.to_ascii_lowercase();
    let numbered = |device: &str| {
        (stem.strip_prefix(device)).is_some_and(|rest| matches!(rest.as_bytes(), [b'0'..=b'9']))
    };
    let device = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || numbered("com")
        || numbered("lpt");
    !name.is_empty()
        && !name.starts_with('.')
        && (name.bytes()).all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        && !device
}

/// The SHA-256 of `bytes` in lowercase hex, as the list names a package's.
pub fn sha256(bytes: &[u8]) -> String {
    let mut sum = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(sum, "{byte:02x}");
    }
    sum
}

/// The `api` of the manifest the package `bytes` holds: the interface version its plugin is
/// built for.
fn built_for(bytes: &[u8]) -> Option<String> {
    let mut archive = Archive::new(GzDecoder::new(bytes));
    let mut manifest = (archive.entries().ok()?)
        .filter_map(Result::ok)
        .find(|entry| entry.path().is_ok_and(|path| path == Path::new(MANIFEST)))?;
    let mut text = String::new();
    manifest.read_to_string(&mut text).ok()?;
    let manifest: DocumentMut = text.parse().ok()?;
    Some(manifest.get("api")?.as_str()?.to_owned())
}

/// What is wrong with the list and the sources under `root`, one line each: a listed package
/// that is missing, has another SHA-256 or another address than its id and version give, or
/// whose manifest names another `api` than the list; a source that does not pack; a source
/// that changed under a version already packed; a new version that is not later than the
/// listed one; and a pack that names a plugin the list does not offer yet. A new plugin's
/// entry names no `version` until `pack` lists it, and a source the list does not name
/// ([`unlisted`]) only has to pack.
pub fn check(root: &Path) -> Result<Vec<String>, String> {
    let list = read(&root.join(LIST))?;
    let sources = sources(root)?;
    let mut wrong = Vec::new();
    for plugin in tables(&list, "plugin") {
        let word = |key: &str| plugin.get(key).and_then(|item| item.as_str());
        let id = word("id").unwrap_or("");
        let Some(version) = word("version") else {
            if !sources.iter().any(|source| source.id == id) {
                wrong.push(format!("{id}: no `version`, and no source to pack"));
            }
            continue;
        };
        let file = format!("{id}-{version}.kidouplug");
        if word("package") != Some(&format!("{PACKAGES}/{file}")) {
            wrong.push(format!("{id}: `package` is not {PACKAGES}/{file}"));
        }
        match fs::read(root.join("packages").join(&file)) {
            Ok(bytes) if Some(sha256(&bytes).as_str()) == word("sha256") => {
                match built_for(&bytes) {
                    Some(api) if Some(api.as_str()) == word("api") => {}
                    Some(api) if word("api").is_none() => wrong.push(format!(
                        "{id}: no `api`; the manifest in packages/{file} names {api}"
                    )),
                    Some(api) => wrong.push(format!(
                        "{id}: `api` is not {api}, which the manifest in packages/{file} names"
                    )),
                    None => wrong.push(format!(
                        "{id}: the manifest in packages/{file} names no `api`"
                    )),
                }
            }
            Ok(_) => wrong.push(format!("{id}: `sha256` is not that of packages/{file}")),
            Err(why) => wrong.push(format!("{id}: packages/{file}: {why}")),
        }
    }
    for source in &sources {
        let id = &source.id;
        let listed = entry(&list, id);
        let word = |key: &str| listed?.get(key)?.as_str();
        if listed.is_some() && word("category") != Some(source.category) {
            wrong.push(format!("{id}: the list names another category"));
        }
        let packed = match source.pack() {
            Ok(packed) => packed,
            Err(why) => {
                wrong.push(why);
                continue;
            }
        };
        if listed.is_none() {
            continue;
        }
        let version = word("version");
        match fs::read(root.join("packages").join(source.package())) {
            Ok(listed) if listed != packed => wrong.push(format!(
                "{id}: the source differs from packages/{}: a package never changes, so give \
                 the plugin a new version",
                source.package()
            )),
            Ok(_) if version != Some(&source.version) => wrong.push(format!(
                "{id}: the list names {}, the source {}",
                version.unwrap_or("no version"),
                source.version
            )),
            Err(_) if version.and_then(numbers) >= numbers(&source.version) => {
                wrong.push(format!(
                    "{id}: the new version {} is not later than the listed {}",
                    source.version,
                    version.unwrap_or_default()
                ));
            }
            Ok(_) | Err(_) => {}
        }
    }
    for pack in tables(&list, "pack") {
        let name = pack.get("id").and_then(|item| item.as_str()).unwrap_or("");
        let ids = pack.get("plugins").and_then(|item| item.as_array());
        for id in ids.into_iter().flatten().filter_map(|id| id.as_str()) {
            let version = entry(&list, id).and_then(|plugin| plugin.get("version")?.as_str());
            if version.is_none() {
                wrong.push(format!("pack {name}: the list offers no plugin `{id}`"));
            }
        }
    }
    Ok(wrong)
}

/// What Kidō would refuse in the Home layouts under `root`, one line each: a file it does not
/// take, an SVG it does not draw, a rule of the layout check, or one of Slint's errors.
pub fn layouts(root: &Path) -> Result<Vec<String>, String> {
    let mut wrong = Vec::new();
    for source in sources(root)? {
        if source.category != "layout" {
            continue;
        }
        let id = &source.id;
        let read = Files::read(&source.folder.join(kidou_layout::FOLDER)).and_then(|mut files| {
            files.draw(|_, svg| kidou_layout::draw_svg(svg).map_err(str::to_owned))?;
            Ok(files)
        });
        let files = match read {
            Ok(files) => files,
            Err(why) => {
                wrong.push(format!("{id}: {why}"));
                continue;
            }
        };
        let refused = kidou_layout::check(&files)
            .and_then(|()| kidou_layout::compile(&files).map(|_| ()))
            .err();
        wrong.extend((refused.into_iter().flatten()).map(|why| format!("{id}: {why}")));
    }
    Ok(wrong)
}

/// The id of each source under `root` that the list does not name: Kidō does not offer it, and
/// `pack` writes no package of it.
pub fn unlisted(root: &Path) -> Result<Vec<String>, String> {
    let list = read(&root.join(LIST))?;
    let sources = sources(root)?.into_iter().map(|source| source.id);
    Ok(sources.filter(|id| entry(&list, id).is_none()).collect())
}

/// Writes the package of each listed source under `root` whose version has none, and gives the
/// list's entry its version, `api`, address and SHA-256. Gives each plugin packed, as
/// `<id> <version>`. Writes nothing where one of them does not pack.
pub fn pack_new(root: &Path) -> Result<Vec<String>, String> {
    let at = root.join(LIST);
    let mut list = read(&at)?;
    let mut new = Vec::new();
    for source in sources(root)? {
        let file = root.join("packages").join(source.package());
        if file.exists() {
            continue;
        }
        let Some(entry) = (list.get_mut("plugin"))
            .and_then(|plugins| plugins.as_array_of_tables_mut())
            .and_then(|plugins| {
                (plugins.iter_mut())
                    .find(|plugin| plugin.get("id").and_then(|id| id.as_str()) == Some(&source.id))
            })
        else {
            continue;
        };
        let bytes = source.pack()?;
        entry["version"] = value(&source.version);
        entry["api"] = value(&source.api);
        entry["package"] = value(format!("{PACKAGES}/{}", source.package()));
        entry["sha256"] = value(sha256(&bytes));
        new.push((file, bytes, format!("{} {}", source.id, source.version)));
    }
    let mut packed = Vec::new();
    for (file, bytes, line) in new {
        fs::write(&file, bytes).map_err(|why| format!("{}: {why}", file.display()))?;
        packed.push(line);
    }
    if !packed.is_empty() {
        fs::write(&at, list.to_string()).map_err(|why| format!("{}: {why}", at.display()))?;
    }
    Ok(packed)
}

fn read(file: &Path) -> Result<DocumentMut, String> {
    let at = |why: String| format!("{}: {why}", file.display());
    let text = fs::read_to_string(file).map_err(|why| at(why.to_string()))?;
    text.parse()
        .map_err(|why: toml_edit::TomlError| at(why.to_string()))
}

/// The list's entry of the plugin `id`.
fn entry<'a>(list: &'a DocumentMut, id: &str) -> Option<&'a Table> {
    tables(list, "plugin").find(|plugin| plugin.get("id").and_then(|id| id.as_str()) == Some(id))
}

fn tables<'a>(list: &'a DocumentMut, key: &str) -> impl Iterator<Item = &'a Table> {
    (list.get(key).and_then(|item| item.as_array_of_tables()))
        .into_iter()
        .flatten()
}
