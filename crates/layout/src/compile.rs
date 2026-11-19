//! The layout compiled by Slint's interpreter from the copy in memory, under a root Home owns,
//! and the layout Home loads: that copy matched with its record first.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use slint_interpreter::{Compiler, ComponentDefinition, DiagnosticLevel};

use crate::{Files, Mismatch, Record, block_on, source};

/// Why Home shows Kidō's own layout in place of a plugin's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// No readable record: `kidouplug` switched the layout off, or never recorded it. It shows
    /// again once it is installed again.
    Off,
    /// A record of another version of Kidō: `kidouplug rebuild` checks the layout again, and
    /// writes its record.
    Unchecked,
    /// Its files changed after the check.
    Changed,
    /// Its folder cannot be read, or it does not compile.
    Broken(Vec<String>),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Off => f.write_str(
                "it has no record of its check, so it is off until it is installed again",
            ),
            Refused::Unchecked => {
                f.write_str("its record is of another version of Kidō; kidouplug checks it again")
            }
            Refused::Changed => f.write_str(
                "its files changed after the check, so it is off until it is installed again",
            ),
            Refused::Broken(why) => write!(
                f,
                "it cannot be read or does not compile: {}",
                why.join("; ")
            ),
        }
    }
}

/// The layout of the plugin whose folder is `plugin`, as Home shows it: its files and drawn PNGs
/// read into memory ([`Files::installed`]), matched with the record at `record` of the Kidō of `version`, and compiled
/// from that copy, which is all Home reads of it.
pub fn load(plugin: &Path, record: &Path, version: &str) -> Result<ComponentDefinition, Refused> {
    let record = std::fs::read_to_string(record)
        .ok()
        .and_then(|text| Record::parse(&text))
        .ok_or(Refused::Off)?;
    let files = Files::installed(plugin).map_err(|why| Refused::Broken(vec![why]))?;
    record
        .matches(&files, version)
        .map_err(|mismatch| match mismatch {
            Mismatch::Version => Refused::Unchecked,
            Mismatch::Files => Refused::Changed,
        })?;
    compile(&files).map_err(Refused::Broken)
}

/// The root Home embeds: a window with no background of its own, so that the layout can neither
/// paint the window's background nor claim the window, which holds the layout and binds the
/// contract's properties to the layout's. A layout that lacks one, or has it of another type, does
/// not compile.
const WRAPPER: (&str, &str) = (
    "@root.slint",
    r#"
import {
    LayoutTile, LayoutHero, LayoutEmptyWords, LayoutTheme, LayoutZone, LayoutPlace,
} from "kidou-layout-3.slint";
import { Layout } from "layout.slint";
export component Root inherits Window {
    background: transparent;
    in property <[LayoutTile]> tiles <=> layout.tiles;
    in property <int> focused <=> layout.focused;
    in property <LayoutZone> zone <=> layout.zone;
    in property <image> stage <=> layout.stage;
    in property <LayoutHero> hero <=> layout.hero;
    in property <bool> empty <=> layout.empty;
    in property <LayoutEmptyWords> empty-words <=> layout.empty-words;
    in property <[string]> tabs <=> layout.tabs;
    in property <int> tab <=> layout.tab;
    in property <LayoutTheme> theme <=> layout.theme;
    out property <length> actions-x <=> layout.actions-x;
    out property <length> actions-y <=> layout.actions-y;
    out property <LayoutPlace> focused-place <=> layout.focused-place;
    callback activated(int);
    callback tab-tapped(int);
    layout := Layout {
        width: 100%;
        height: 100%;
        activated(index) => { root.activated(index); }
        tab-tapped(index) => { root.tab-tapped(index); }
    }
}
"#,
);

/// The layout `files`, compiled under Home's root `Root`, or Slint's errors.
pub fn compile(files: &Files) -> Result<ComponentDefinition, Vec<String>> {
    let mut compiler = Compiler::default();
    let images = Rc::new(RefCell::new(source::Images::default()));
    source::configure(
        compiler.compiler_configuration(i_slint_core::InternalToken),
        Rc::new(files.clone()),
        images.clone(),
    );
    compiler.set_resource_loader(
        |path| {
            Box::pin(async move {
                Err(std::io::Error::other(format!(
                    "{path}: a layout uses Kidō's fonts"
                )))
            })
        },
        i_slint_core::InternalToken,
    );
    let result = block_on(compiler.build_from_source_path(
        Some(WRAPPER.1.to_owned()),
        source::address(WRAPPER.0),
        i_slint_core::InternalToken,
    ));
    let mut errors: Vec<String> = result
        .diagnostics()
        .filter(|found| found.level() == DiagnosticLevel::Error)
        .map(|found| format!("slint: {}", found.message()))
        .collect();
    errors.extend(images.take().refused);
    match result.component("Root") {
        Some(definition) if errors.is_empty() => Ok(definition),
        _ if errors.is_empty() => Err(vec!["slint: no root".to_owned()]),
        _ => Err(errors),
    }
}
