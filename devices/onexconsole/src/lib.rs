//! OneXConsole, the OneXPlayer's maker's app, as a device plugin with the manifest `plugin.toml`.
//! It reads OneXConsole's database `core.db` and never writes it; the tables are those of
//! OneXConsole 0.10.4-fix1. It reads the file as it is, without SQLite's lock, so that
//! OneXConsole's own writes never wait on it; a read during a write can fail or see half of it,
//! and the next change in the folder reads again.
//!
//! - The tile `presets` states how many presets there are; a press shows each preset with the
//!   programs that start it.
//! - The tile `open` opens OneXConsole, where the player changes and binds them.

#[cfg(target_arch = "wasm32")]
#[allow(clippy::same_length_and_capacity)]
mod plugin {
    wit_bindgen::generate!({
        world: "kidou:device/device@1.0.0",
        path: [
            "../../wit/deps/host",
            "../../wit/events",
            "../../wit/device",
        ],
        generate_all,
    });

    use exports::kidou::device::handler::{Context, Guest};
    use kidou::events::types::{Action, Language, Launch, Pressed, Program, Sheet};
    use kidou::host::sqlite::{Access, Database, Value};

    /// The granted folders, as the manifest writes them.
    const DATABASE: &str = r"{AppData}\gamerzone\db";
    const PROGRAM: &str = r"{ProgramFiles}\OneXConsole";

    /// The process name OneXConsole binds the preset to that applies while no bound game runs.
    const DESKTOP: &str = "__virtual_desktop";

    struct OneXConsole;

    struct Preset {
        id: i64,
        title: String,
        default: bool,
        programs: Vec<String>,
    }

    fn folder<'a>(context: &'a Context, written: &str) -> Option<&'a str> {
        (context.folders.iter())
            .find(|(text, _)| text == written)
            .map(|(_, path)| path.as_str())
    }

    fn text(value: Option<&Value>) -> String {
        match value {
            Some(Value::Text(text)) => text.clone(),
            Some(Value::Integer(number)) => number.to_string(),
            _ => String::new(),
        }
    }

    fn integer(value: Option<&Value>) -> i64 {
        match value {
            Some(Value::Integer(number)) => *number,
            _ => 0,
        }
    }

    /// The presets in OneXConsole's order, or why they cannot be read.
    fn presets(context: &Context) -> Result<Vec<Preset>, String> {
        let folder = folder(context, DATABASE).ok_or("no folder")?;
        let database = Database::open(&format!("{folder}/core.db"), Access::Immutable)
            .map_err(|error| error.message)?;
        let rows = database
            .query(
                "SELECT id, title, isDefault FROM game_setting_preset ORDER BY showIndex, id",
                &[],
            )
            .map_err(|error| error.message)?;
        let mut presets: Vec<Preset> = rows
            .iter()
            .map(|row| Preset {
                id: integer(row.first()),
                title: text(row.get(1)),
                default: integer(row.get(2)) != 0,
                programs: Vec::new(),
            })
            .collect();
        let bound = database
            .query(
                "SELECT presetId, processName FROM game_setting_preset_process ORDER BY id",
                &[],
            )
            .map_err(|error| error.message)?;
        for row in &bound {
            let id = integer(row.first());
            if let Some(preset) = presets.iter_mut().find(|preset| preset.id == id) {
                preset.programs.push(text(row.get(1)));
            }
        }
        Ok(presets)
    }

    fn german(context: &Context) -> bool {
        context.language == Language::German
    }

    fn count(context: &Context, presets: &Result<Vec<Preset>, String>) -> String {
        let de = german(context);
        match presets {
            Err(_) if de => "Nicht gefunden".to_owned(),
            Err(_) => "Not found".to_owned(),
            Ok(presets) => match (presets.len(), de) {
                (1, true) => "1 Vorgabe".to_owned(),
                (1, false) => "1 preset".to_owned(),
                (n, true) => format!("{n} Vorgaben"),
                (n, false) => format!("{n} presets"),
            },
        }
    }

    /// A preset's row: its title, and the programs that start it.
    fn row(context: &Context, preset: &Preset) -> (String, String) {
        let de = german(context);
        let title = match (preset.title.trim(), preset.default, de) {
            ("", true, true) => "Standard".to_owned(),
            ("", true, false) => "Default".to_owned(),
            ("", false, true) => format!("Vorgabe {}", preset.id),
            ("", false, false) => format!("Preset {}", preset.id),
            (title, ..) => title.to_owned(),
        };
        let programs: Vec<&str> = (preset.programs.iter())
            .map(|program| match (program.as_str(), de) {
                (DESKTOP, true) => "Desktop-Prozess",
                (DESKTOP, false) => "Desktop process",
                (program, _) => program,
            })
            .collect();
        let programs = match (programs.is_empty(), de) {
            (true, true) => "Keinem Programm zugeordnet".to_owned(),
            (true, false) => "Bound to no program".to_owned(),
            (false, _) => programs.join(", "),
        };
        (title, programs)
    }

    fn open(context: &Context) -> Option<Action> {
        let folder = folder(context, PROGRAM)?;
        Some(Action::Open(Launch::Program(Program {
            program: format!("{folder}/OneXConsole.exe"),
            arguments: String::new(),
            directory: folder.to_owned(),
            within: folder.to_owned(),
        })))
    }

    impl Guest for OneXConsole {
        fn states(context: Context) -> Vec<(String, String)> {
            vec![("presets".to_owned(), count(&context, &presets(&context)))]
        }

        fn press(tile: String, context: Context) -> Pressed {
            match tile.as_str() {
                "presets" => {
                    let presets = presets(&context);
                    let heading = if german(&context) {
                        "Vorgaben in OneXConsole"
                    } else {
                        "OneXConsole presets"
                    };
                    Pressed {
                        action: Some(Action::Sheet(Sheet {
                            title: heading.to_owned(),
                            rows: (presets.iter().flatten())
                                .map(|preset| row(&context, preset))
                                .collect(),
                        })),
                        state: Some(count(&context, &presets)),
                    }
                }
                "open" => Pressed {
                    action: open(&context),
                    state: None,
                },
                _ => Pressed {
                    action: None,
                    state: None,
                },
            }
        }
    }

    export!(OneXConsole);
}
