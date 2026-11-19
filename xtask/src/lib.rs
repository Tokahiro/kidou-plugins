//! Kidō's plugin list held to the sources of this repository: `check` finds what differs, and
//! `pack` writes the package of each source whose version has none and lists it.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use flate2::{Compression, write::GzEncoder};
use sha2::{Digest, Sha256};
use tar::{Builder, EntryType, Header};
use toml_edit::{DocumentMut, Table, value};

/// Where Kidō reads a package of this repository.
pub const PACKAGES: &str = "https://raw.githubusercontent.com/Tokahiro/kidou-plugins/main/packages";

/// Each category whose plugins have their source here: its name, the folder that holds them,
/// one folder per plugin named by its id, and the folder a plugin of it brings beside its
/// manifest.
pub const SOURCES: [(&str, &str, &str); 1] = [("layout", "layouts", "layout")];

const LIST: &str = "index.toml";
const MANIFEST: &str = "plugin.toml";

/// A plugin whose source this repository holds.
#[derive(Debug)]
pub struct Source {
    pub folder: PathBuf,
    pub id: String,
    pub version: String,
    pub category: &'static str,
    brings: &'static str,
}

impl Source {
    /// The name of its version's package file.
    pub fn package(&self) -> String {
        format!("{}-{}.kidouplug", self.id, self.version)
    }

    /// Its package, as `kidouplug pack` writes it: a gzip'd tar of the manifest and the folder
    /// its category brings, each file and folder by name and each folder before what it holds.
    /// The same source gives the same bytes.
    pub fn pack(&self) -> Result<Vec<u8>, String> {
        let manifest = self.folder.join(MANIFEST);
        let text = fs::read(&manifest).map_err(|why| format!("{}: {why}", manifest.display()))?;
        let mut files = vec![
            (MANIFEST.to_owned(), Some(text)),
            (self.brings.to_owned(), None),
        ];
        walk(&self.folder, self.brings, &mut files)?;
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
}

/// Each plugin with its source under `root`, by category and id; refuses a manifest whose id is
/// not its folder's name, whose category is not its folder's, or whose version is not three
/// numbers.
pub fn sources(root: &Path) -> Result<Vec<Source>, String> {
    let mut found = Vec::new();
    for (category, folder, brings) in SOURCES {
        let Ok(entries) = fs::read_dir(root.join(folder)) else {
            continue;
        };
        let mut folders: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
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
            if word("category")? != category {
                return Err(format!("{}: no `{category}` plugin", at.display()));
            }
            let numbers = version
                .split('.')
                .filter(|part| part.parse::<u32>().is_ok());
            if version.split('.').count() != 3 || numbers.count() != 3 {
                return Err(format!(
                    "{}: `{version}` is not three numbers",
                    at.display()
                ));
            }
            found.push(Source {
                folder,
                id,
                version,
                category,
                brings,
            });
        }
    }
    Ok(found)
}

fn walk(folder: &Path, at: &str, files: &mut Vec<(String, Option<Vec<u8>>)>) -> Result<(), String> {
    let path = folder.join(at);
    let unreadable = |why: std::io::Error| format!("{}: {why}", path.display());
    let mut names = Vec::new();
    for entry in fs::read_dir(&path).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !fit(&name) {
            return Err(format!(
                "{}: {name:?} is no name a package may hold: only [A-Za-z0-9._-], not starting \
                 with a dot",
                path.display()
            ));
        }
        names.push((name, entry.file_type().map_err(unreadable)?));
    }
    names.sort_by(|(one, _), (other, _)| one.cmp(other));
    for (name, kind) in names {
        let inner = format!("{at}/{name}");
        if kind.is_dir() {
            files.push((inner.clone(), None));
            walk(folder, &inner, files)?;
        } else if kind.is_file() {
            let bytes = fs::read(folder.join(&inner)).map_err(unreadable)?;
            files.push((inner, Some(bytes)));
        } else {
            return Err(format!("{}: {inner} is a link", folder.display()));
        }
    }
    Ok(())
}

/// Whether Kidō unpacks a file or folder of this name.
fn fit(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && (name.bytes()).all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

/// The SHA-256 of `bytes` in lowercase hex, as the list names a package's.
pub fn sha256(bytes: &[u8]) -> String {
    let mut sum = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(sum, "{byte:02x}");
    }
    sum
}

/// What is wrong with the list and the sources under `root`, one line each: a listed package
/// that is missing, has another SHA-256 or another address than its id and version give; a
/// source the list does not name; a source that changed under a version already packed; and a
/// pack that names a plugin the list does not offer.
pub fn check(root: &Path) -> Result<Vec<String>, String> {
    let list = read(&root.join(LIST))?;
    let mut wrong = Vec::new();
    let mut listed = Vec::new();
    for plugin in tables(&list, "plugin") {
        let word = |key: &str| plugin.get(key).and_then(|item| item.as_str()).unwrap_or("");
        let (id, version) = (word("id"), word("version"));
        listed.push((id, version, word("category")));
        let file = format!("{id}-{version}.kidouplug");
        if word("package") != format!("{PACKAGES}/{file}") {
            wrong.push(format!("{id}: `package` is not {PACKAGES}/{file}"));
        }
        match fs::read(root.join("packages").join(&file)) {
            Ok(bytes) if sha256(&bytes) == word("sha256") => {}
            Ok(_) => wrong.push(format!("{id}: `sha256` is not that of packages/{file}")),
            Err(why) => wrong.push(format!("{id}: packages/{file}: {why}")),
        }
    }
    for source in sources(root)? {
        let id = &source.id;
        let Some((_, version, category)) = listed.iter().find(|(listed, ..)| listed == id) else {
            wrong.push(format!("{id}: the list has no [[plugin]] of this id"));
            continue;
        };
        if *category != source.category {
            wrong.push(format!("{id}: the list names another category"));
        }
        let Ok(packed) = fs::read(root.join("packages").join(source.package())) else {
            continue;
        };
        if packed != source.pack()? {
            wrong.push(format!(
                "{id}: the source differs from packages/{}: a package never changes, so give \
                 the plugin a new version",
                source.package()
            ));
        } else if *version != source.version {
            wrong.push(format!(
                "{id}: the list names {version}, the source {}",
                source.version
            ));
        }
    }
    for pack in tables(&list, "pack") {
        let name = pack.get("id").and_then(|item| item.as_str()).unwrap_or("");
        let ids = pack.get("plugins").and_then(|item| item.as_array());
        for id in ids.into_iter().flatten().filter_map(|id| id.as_str()) {
            if !listed.iter().any(|(listed, ..)| *listed == id) {
                wrong.push(format!("pack {name}: the list offers no plugin `{id}`"));
            }
        }
    }
    Ok(wrong)
}

/// Writes the package of each source under `root` whose version has none, and gives the list's
/// entry its version, address and SHA-256. Gives each plugin packed, as `<id> <version>`.
pub fn pack_new(root: &Path) -> Result<Vec<String>, String> {
    let at = root.join(LIST);
    let mut list = read(&at)?;
    let mut packed = Vec::new();
    for source in sources(root)? {
        let file = root.join("packages").join(source.package());
        if file.exists() {
            continue;
        }
        let bytes = source.pack()?;
        let entry = (list.get_mut("plugin"))
            .and_then(|plugins| plugins.as_array_of_tables_mut())
            .and_then(|plugins| {
                (plugins.iter_mut())
                    .find(|plugin| plugin.get("id").and_then(|id| id.as_str()) == Some(&source.id))
            })
            .ok_or_else(|| format!("{}: the list has no [[plugin]] of this id", source.id))?;
        entry["version"] = value(&source.version);
        entry["package"] = value(format!("{PACKAGES}/{}", source.package()));
        entry["sha256"] = value(sha256(&bytes));
        fs::write(&file, bytes).map_err(|why| format!("{}: {why}", file.display()))?;
        packed.push(format!("{} {}", source.id, source.version));
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

fn tables<'a>(list: &'a DocumentMut, key: &str) -> impl Iterator<Item = &'a Table> {
    (list.get(key).and_then(|item| item.as_array_of_tables()))
        .into_iter()
        .flatten()
}
