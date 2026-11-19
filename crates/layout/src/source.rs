//! The one way Slint's compiler and interpreter reach a layout's files: each file has an
//! address under [`BASE`], which only the loaders below answer, from the copy in memory. A
//! path on the disk, another address or a file the copy does not hold is refused, so nothing
//! is read from the disk: no `.slint` file, image, font or project file.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::rc::Rc;

use i_slint_compiler::source_path::SourcePath;
use i_slint_compiler::{CompilerConfiguration, ExperimentalFeatures};
use url::Url;

use base64::Engine;

use crate::Files;
use crate::files::shown;

/// The address under which a layout's files lie.
const BASE: &str = "kidou-layout://package/";

/// Where an image the layout names but may not show goes: an empty `data:` address, which
/// Slint draws as no image.
pub(crate) const NO_IMAGE: &str = "data:,";

/// What the resource mapper did with the images the layout names.
#[derive(Default)]
pub(crate) struct Images {
    /// Why each refused image is refused.
    pub refused: Vec<String>,
    /// The `data:` address each image of the layout became, with the pixels it decodes to.
    pub mapped: HashMap<String, u64>,
}

/// The address of the layout's file `name`, one of this crate's own names.
pub(crate) fn address(name: &str) -> SourcePath {
    let url = Url::parse(BASE).and_then(|base| base.join(name));
    SourcePath::from_url(&url.expect("BASE and the crate's names make an address"))
}

/// The name of the layout's file at the address `url`, where it is one.
fn name_of(url: &Url) -> Option<&str> {
    url.as_str()
        .strip_prefix(BASE)
        .filter(|name| !name.is_empty())
}

/// Sets `config` to read only `files`, and the files Home serves from memory; `images` collects
/// what became of each image the layout names by a path.
pub(crate) fn configure(
    config: &mut CompilerConfiguration,
    files: Rc<Files>,
    images: Rc<RefCell<Images>>,
) {
    config.include_paths.clear();
    config.library_paths.clear();
    config.style = Some("fluent".into());
    config.experimental_features = ExperimentalFeatures::default();
    config.unknown_experimental_features.clear();
    let texts = files.clone();
    config.open_import_callback = Some(Rc::new(move |path: SourcePath| {
        let loaded = match &path {
            SourcePath::Builtin(_) => None,
            SourcePath::Url(url) => Some(match name_of(url) {
                Some(name) => match (crate::served(name), name.rsplit_once('.')) {
                    (Some(text), _) => Ok(text.to_owned()),
                    (None, Some((_, "slint"))) => match texts.get(name) {
                        Some(bytes) => String::from_utf8(bytes.to_vec())
                            .map_err(|_| io::Error::other(format!("{name}: no UTF-8 text"))),
                        None => Err(not_in_layout(&path)),
                    },
                    _ => Err(not_in_layout(&path)),
                },
                None => Err(not_in_layout(&path)),
            }),
            SourcePath::File(_) => Some(Err(not_in_layout(&path))),
        };
        Box::pin(async move { loaded }) as Pin<Box<dyn Future<Output = Option<io::Result<String>>>>>
    }));
    config.resource_url_mapper = Some(Rc::new(move |url: &Url| {
        let mapped = if url.scheme() == "builtin" {
            None
        } else {
            let mapped = name_of(url)
                .ok_or_else(|| format!("{url}: an image that is not in the layout"))
                .and_then(|name| {
                    let shown = shown(name, &files)?;
                    let data = format!(
                        "data:{};base64,{}",
                        shown.kind,
                        base64::engine::general_purpose::STANDARD.encode(shown.bytes)
                    );
                    let data = Url::parse(&data).map_err(|e| format!("{name}: {e}"))?;
                    Ok((data, shown.pixels))
                });
            let mut images = images.borrow_mut();
            match mapped {
                Ok((data, pixels)) => {
                    images.mapped.insert(data.as_str().to_owned(), pixels);
                    Some(data)
                }
                Err(why) => {
                    images.refused.push(why);
                    Url::parse(NO_IMAGE).ok()
                }
            }
        };
        Box::pin(async move { mapped }) as Pin<Box<dyn Future<Output = Option<Url>>>>
    }));
}

fn not_in_layout(path: &SourcePath) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{path}: not a file of the layout"),
    )
}
