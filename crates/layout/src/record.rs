//! The record `kidouplug` writes once a layout passed the check: the sum of its files and the
//! PNGs drawn of its SVGs ([`Files::sum`]), which
//! covers their names, and the version of Kidō that checked it. Home shows a layout only while
//! its files still match the record of its own version.

use std::path::{Path, PathBuf};

use crate::Files;

/// The first line of a record, which names its format.
const HEADER: &str = "kidou-layout-record 1";

/// The record of the layout of the plugin `id`, in Kidō's data folder `data`: beside the
/// plugins, never in the package, which a player or a program could change.
pub fn record_path(data: &Path, id: &str) -> PathBuf {
    data.join("layouts").join(format!("{id}.record"))
}

/// A layout's record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// The version of Kidō that checked the layout, as its caller names it.
    pub version: String,
    pub sum: String,
}

/// Why a layout's files do not match its record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mismatch {
    /// Another version of Kidō checked it: `kidouplug rebuild` checks it again.
    Version,
    /// Its files changed after the check.
    Files,
}

impl Record {
    /// The record of `files`, checked by the Kidō of `version`.
    pub fn of(files: &Files, version: &str) -> Self {
        Self {
            version: version.to_owned(),
            sum: files.sum(),
        }
    }

    /// Whether `files` are those this record was written of, by the Kidō of `version`.
    pub fn matches(&self, files: &Files, version: &str) -> Result<(), Mismatch> {
        if self.version != version {
            return Err(Mismatch::Version);
        }
        if self.sum != files.sum() {
            return Err(Mismatch::Files);
        }
        Ok(())
    }

    /// The record as its file holds it.
    pub fn text(&self) -> String {
        format!("{HEADER}\nversion {}\nsum {}\n", self.version, self.sum)
    }

    /// The record of the file text `text`; `None` where it is no record of this format.
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        if lines.next()? != HEADER {
            return None;
        }
        let version = lines.next()?.strip_prefix("version ")?.to_owned();
        let sum = lines.next()?.strip_prefix("sum ")?.to_owned();
        lines.next().is_none().then_some(Self { version, sum })
    }
}
