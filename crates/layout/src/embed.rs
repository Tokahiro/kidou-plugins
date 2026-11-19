//! A compiled layout embedded in Home's window: Home's `ComponentContainer` makes it through
//! [`Embedded::factory`], and Home gives it its row, stage, hero, empty card, tabs and theme
//! through [`Embedded::give`], as the contract's root properties, and learns where the layout
//! wants the hero's capsules.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use i_slint_core::component_factory::{ComponentFactory, FactoryContext};
use i_slint_core::model::{Model, ModelRc, VecModel};
use i_slint_core::properties::ChangeTracker;
use slint_interpreter::{
    Brush, Color, ComponentDefinition, ComponentHandle, ComponentInstance, Image, SharedString,
    Struct, Value,
};

/// A game of Home's row, as the contract's `LayoutTile`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tile {
    pub title: SharedString,
    pub badge: SharedString,
    pub art: Image,
    pub running: bool,
    pub missing: bool,
    pub shows_title: bool,
    pub title_length: TitleLength,
    /// A program's icon; an image of no width for a game.
    pub icon: Image,
    pub tint: Color,
}

/// How long a title is, as the contract's `LayoutTitleLength`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TitleLength {
    #[default]
    Short,
    Medium,
    Long,
}

/// A fact of the focused game, as the contract's `LayoutFact`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fact {
    pub icon: Image,
    pub icon_color: Color,
    pub text: SharedString,
}

/// The focused game's texts, as the contract's `LayoutHero`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Hero {
    pub shown: bool,
    pub kicker: SharedString,
    pub title: SharedString,
    pub facts: SharedString,
    pub fact_list: Vec<Fact>,
    pub tagline: SharedString,
    pub logo: Image,
    pub up_next: SharedString,
    pub recently_played: SharedString,
}

/// The words of the card that stands for an empty library, as the contract's `LayoutEmptyWords`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EmptyWords {
    pub title: SharedString,
    pub body: SharedString,
    pub scan_again: SharedString,
}

/// Home's colors, units and lengths, as the contract's `LayoutTheme`; lengths in logical pixels.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Theme {
    pub text: Color,
    pub muted: Color,
    pub text_secondary: Color,
    pub accent: Color,
    pub accent_label: Color,
    pub accent_mid: Color,
    pub glow_alpha_soft: f32,
    pub focus_halo_soft: Color,
    pub white_08: Color,
    pub white_20: Color,
    pub segment_tint: Brush,
    pub segment_border: Color,
    pub ok: Color,
    pub missing_art: Brush,
    /// This and `library_symbol` are images of no width where the player turned symbols off.
    pub world_symbol: Image,
    pub library_symbol: Image,
    pub glass_clear: bool,
    pub glass_tint: Brush,
    pub glass_rim: Color,
    pub glass_highlight: Color,
    pub glass_bezel: f32,
    pub glass_refraction: f32,
    pub glass_dispersion: f32,
    pub glass_edge_light: f32,
    pub glass_capsule_from: Color,
    pub glass_capsule_to: Color,
    pub glass_panel_from: Color,
    pub glass_panel_to: Color,
    pub opacity_dimmed_tile: f32,
    pub opacity_dock_art: f32,
    pub radius_cover: f32,
    pub focus_lift_cover: f32,
    pub focus_grow_cover: f32,
    pub canvas_margin: f32,
    pub content_left: f32,
    pub content_right: f32,
    pub under_switcher: f32,
    pub unit: f32,
    pub text_scale: f32,
    pub cards: CardSize,
    pub safe_lift: f32,
    pub lane_crest: f32,
    pub lane_bend_top: f32,
    pub lane_bend: f32,
    pub reduced_motion: bool,
}

/// Home's Kartengröße, as the contract's `LayoutCardSize`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CardSize {
    Normal,
    #[default]
    Large,
    VeryLarge,
}

/// Where Home's focus is, as the contract's `LayoutZone`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Zone {
    #[default]
    Row,
    Actions,
    Tabs,
    Other,
}

/// All Home gives a layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Given {
    pub tiles: Vec<Tile>,
    pub focused: i32,
    pub zone: Zone,
    pub stage: Image,
    pub hero: Hero,
    pub empty: bool,
    pub empty_words: EmptyWords,
    pub tabs: Vec<SharedString>,
    pub tab: i32,
    pub theme: Theme,
}

/// Where a tile stands, from the layout's top left, as the contract's `LayoutPlace`; in logical
/// pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Place {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub radius: f32,
}

/// Where a layout wants Home's hero's capsules, and where its focused tile stands.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Placed {
    pub actions_x: f32,
    pub actions_y: f32,
    pub focused: Place,
}

/// A compiled layout, and the instance Home's window shows of it once made.
pub struct Embedded(Rc<Shared>);

struct Shared {
    definition: ComponentDefinition,
    instance: RefCell<Option<ComponentInstance>>,
    given: RefCell<Given>,
    tiles: Rc<VecModel<Value>>,
    /// Watches where the instance wants the capsules and where its focused tile stands.
    placed: ChangeTracker,
}

impl Embedded {
    /// The layout [`compile`](crate::compile) or [`load`](crate::load) gave.
    pub fn new(definition: ComponentDefinition) -> Self {
        Self(Rc::new(Shared {
            definition,
            instance: RefCell::new(None),
            given: RefCell::new(Given::default()),
            tiles: Rc::new(VecModel::default()),
            placed: ChangeTracker::default(),
        }))
    }

    /// The factory of a `ComponentContainer`: each call makes the layout anew with what was
    /// given last, a tap on its tile `index` calls `activated(index)`, and a tap on its tab
    /// `index` calls `tab_tapped(index)`. `placed` gets where
    /// the layout wants the hero's capsules and where its focused tile stands, each time either
    /// changes. It makes nothing once this layout is dropped.
    pub fn factory(
        &self,
        activated: impl Fn(i32) + 'static,
        tab_tapped: impl Fn(i32) + 'static,
        placed: impl Fn(Placed) + 'static,
    ) -> ComponentFactory {
        let shared = Rc::downgrade(&self.0);
        let activated = Rc::new(activated);
        let tab_tapped = Rc::new(tab_tapped);
        let placed = Rc::new(placed);
        ComponentFactory::new(move |context: FactoryContext| {
            let shared = Weak::upgrade(&shared)?;
            let instance = shared.definition.create_embedded(context).ok()?;
            set(
                &instance,
                "tiles",
                Value::Model(ModelRc::from(shared.tiles.clone())),
            );
            set_given(&instance, None, &shared.given.borrow());
            on_index(&instance, "activated", activated.clone());
            on_index(&instance, "tab-tapped", tab_tapped.clone());
            let placed = placed.clone();
            shared.placed.init_delayed(
                instance.as_weak(),
                |instance| {
                    instance
                        .upgrade()
                        .map_or_else(Placed::default, |instance| Placed {
                            actions_x: length(&instance, "actions-x"),
                            actions_y: length(&instance, "actions-y"),
                            focused: place(&instance, "focused-place"),
                        })
                },
                move |_, &now| placed(now),
            );
            *shared.instance.borrow_mut() = Some(instance.clone_strong());
            Some(instance)
        })
    }

    /// Whether `given` differs from what the layout was given last.
    pub fn differs(&self, given: &Given) -> bool {
        *self.0.given.borrow() != *given
    }

    /// Gives the layout `given`: sets what changed, a changed tile on its own.
    pub fn give(&self, given: Given) {
        let shared = &self.0;
        let old = shared.given.replace(given);
        let given = shared.given.borrow();
        if old.tiles.len() == given.tiles.len() {
            for (index, (old, new)) in old.tiles.iter().zip(&given.tiles).enumerate() {
                if old != new {
                    shared.tiles.set_row_data(index, tile(new));
                }
            }
        } else {
            shared
                .tiles
                .set_vec(given.tiles.iter().map(tile).collect::<Vec<_>>());
        }
        let instance = shared.instance.borrow();
        if let Some(instance) = instance.as_ref() {
            set_given(instance, Some(&old), &given);
        }
    }
}

/// Sets each root property but the tiles that differs between `old` and `new`; each where `old`
/// is `None`.
fn set_given(instance: &ComponentInstance, old: Option<&Given>, new: &Given) {
    let all = old.is_none();
    let old = old.unwrap_or(new);
    if all || old.focused != new.focused {
        set(instance, "focused", Value::Number(f64::from(new.focused)));
    }
    if all || old.zone != new.zone {
        set(instance, "zone", zone(new.zone));
    }
    if all || old.stage != new.stage {
        set(instance, "stage", Value::Image(new.stage.clone()));
    }
    if all || old.hero != new.hero {
        set(instance, "hero", hero(&new.hero));
    }
    if all || old.empty != new.empty {
        set(instance, "empty", Value::Bool(new.empty));
    }
    if all || old.empty_words != new.empty_words {
        set(instance, "empty-words", empty_words(&new.empty_words));
    }
    if all || old.tabs != new.tabs {
        let tabs = new
            .tabs
            .iter()
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>();
        set(
            instance,
            "tabs",
            Value::Model(ModelRc::new(VecModel::from(tabs))),
        );
    }
    if all || old.tab != new.tab {
        set(instance, "tab", Value::Number(f64::from(new.tab)));
    }
    if all || old.theme != new.theme {
        set(instance, "theme", theme(&new.theme));
    }
}

/// `Root` declares the callback `name` with one `int`, so its one argument is a number.
fn on_index(instance: &ComponentInstance, name: &str, call: Rc<impl Fn(i32) + 'static>) {
    let _ = instance.set_callback(name, move |arguments| {
        if let Some(Value::Number(index)) = arguments.first() {
            call(index_of(*index));
        }
        Value::Void
    });
}

/// Sets `Root`'s property `name`, which [`compile`](crate::compile)'s root declares with the type
/// of `value`.
fn set(instance: &ComponentInstance, name: &str, value: Value) {
    let _ = instance.set_property(name, value);
}

/// `Root`'s length `name`, in logical pixels, which [`compile`](crate::compile)'s root declares;
/// 0 where the layout's is no finite length.
fn length(instance: &ComponentInstance, name: &str) -> f32 {
    match instance.get_property(name) {
        Ok(Value::Number(number)) => finite(number),
        _ => 0.0,
    }
}

/// A length the interpreter gives, in logical pixels; 0 where it is no finite `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a length out of f32's range is no finite one"
)]
fn finite(number: f64) -> f32 {
    let number = number as f32;
    if number.is_finite() { number } else { 0.0 }
}

/// `Root`'s place `name`, which [`compile`](crate::compile)'s root declares; a length that is no
/// finite one is 0.
fn place(instance: &ComponentInstance, name: &str) -> Place {
    let Ok(Value::Struct(place)) = instance.get_property(name) else {
        return Place::default();
    };
    let field = |name: &str| match place.get_field(name) {
        Some(Value::Number(number)) => finite(*number),
        _ => 0.0,
    };
    Place {
        x: field("x"),
        y: field("y"),
        width: field("width"),
        height: field("height"),
        radius: field("radius"),
    }
}

/// A tile's index, which Slint's interpreter passes as a number.
#[expect(
    clippy::cast_possible_truncation,
    reason = "an index of at most a row's games; `as` saturates"
)]
fn index_of(number: f64) -> i32 {
    number as i32
}

fn tile(tile: &Tile) -> Value {
    record([
        ("title", Value::String(tile.title.clone())),
        ("badge", Value::String(tile.badge.clone())),
        ("art", Value::Image(tile.art.clone())),
        ("running", Value::Bool(tile.running)),
        ("missing", Value::Bool(tile.missing)),
        ("shows-title", Value::Bool(tile.shows_title)),
        ("title-length", title_length(tile.title_length)),
        ("icon", Value::Image(tile.icon.clone())),
        ("tint", Value::Brush(Brush::SolidColor(tile.tint))),
    ])
}

fn cards(cards: CardSize) -> Value {
    let name = match cards {
        CardSize::Normal => "normal",
        CardSize::Large => "large",
        CardSize::VeryLarge => "very-large",
    };
    Value::EnumerationValue("LayoutCardSize".to_owned(), name.to_owned())
}

fn title_length(length: TitleLength) -> Value {
    let name = match length {
        TitleLength::Short => "short",
        TitleLength::Medium => "medium",
        TitleLength::Long => "long",
    };
    Value::EnumerationValue("LayoutTitleLength".to_owned(), name.to_owned())
}

fn hero(hero: &Hero) -> Value {
    record([
        ("shown", Value::Bool(hero.shown)),
        ("kicker", Value::String(hero.kicker.clone())),
        ("title", Value::String(hero.title.clone())),
        ("facts", Value::String(hero.facts.clone())),
        (
            "fact-list",
            Value::Model(ModelRc::new(VecModel::from(
                hero.fact_list.iter().map(fact).collect::<Vec<_>>(),
            ))),
        ),
        ("tagline", Value::String(hero.tagline.clone())),
        ("logo", Value::Image(hero.logo.clone())),
        ("up-next", Value::String(hero.up_next.clone())),
        (
            "recently-played",
            Value::String(hero.recently_played.clone()),
        ),
    ])
}

fn fact(fact: &Fact) -> Value {
    record([
        ("icon", Value::Image(fact.icon.clone())),
        (
            "icon-color",
            Value::Brush(Brush::SolidColor(fact.icon_color)),
        ),
        ("text", Value::String(fact.text.clone())),
    ])
}

fn empty_words(words: &EmptyWords) -> Value {
    record([
        ("title", Value::String(words.title.clone())),
        ("body", Value::String(words.body.clone())),
        ("scan-again", Value::String(words.scan_again.clone())),
    ])
}

fn theme(theme: &Theme) -> Value {
    let color = |color: Color| Value::Brush(Brush::SolidColor(color));
    let number = |number: f32| Value::Number(f64::from(number));
    record([
        ("text", color(theme.text)),
        ("muted", color(theme.muted)),
        ("text-secondary", color(theme.text_secondary)),
        ("accent", color(theme.accent)),
        ("accent-label", color(theme.accent_label)),
        ("accent-mid", color(theme.accent_mid)),
        ("glow-alpha-soft", number(theme.glow_alpha_soft)),
        ("focus-halo-soft", color(theme.focus_halo_soft)),
        ("white-08", color(theme.white_08)),
        ("white-20", color(theme.white_20)),
        ("segment-tint", Value::Brush(theme.segment_tint.clone())),
        ("segment-border", color(theme.segment_border)),
        ("ok", color(theme.ok)),
        ("missing-art", Value::Brush(theme.missing_art.clone())),
        ("world-symbol", Value::Image(theme.world_symbol.clone())),
        ("library-symbol", Value::Image(theme.library_symbol.clone())),
        ("glass-clear", Value::Bool(theme.glass_clear)),
        ("glass-tint", Value::Brush(theme.glass_tint.clone())),
        ("glass-rim", color(theme.glass_rim)),
        ("glass-highlight", color(theme.glass_highlight)),
        ("glass-bezel", number(theme.glass_bezel)),
        ("glass-refraction", number(theme.glass_refraction)),
        ("glass-dispersion", number(theme.glass_dispersion)),
        ("glass-edge-light", number(theme.glass_edge_light)),
        ("glass-capsule-from", color(theme.glass_capsule_from)),
        ("glass-capsule-to", color(theme.glass_capsule_to)),
        ("glass-panel-from", color(theme.glass_panel_from)),
        ("glass-panel-to", color(theme.glass_panel_to)),
        ("opacity-dimmed-tile", number(theme.opacity_dimmed_tile)),
        ("opacity-dock-art", number(theme.opacity_dock_art)),
        ("radius-cover", number(theme.radius_cover)),
        ("focus-lift-cover", number(theme.focus_lift_cover)),
        ("focus-grow-cover", number(theme.focus_grow_cover)),
        ("canvas-margin", number(theme.canvas_margin)),
        ("content-left", number(theme.content_left)),
        ("content-right", number(theme.content_right)),
        ("under-switcher", number(theme.under_switcher)),
        ("unit", number(theme.unit)),
        ("text-scale", number(theme.text_scale)),
        ("cards", cards(theme.cards)),
        ("safe-lift", number(theme.safe_lift)),
        ("lane-crest", number(theme.lane_crest)),
        ("lane-bend-top", number(theme.lane_bend_top)),
        ("lane-bend", number(theme.lane_bend)),
        ("reduced-motion", Value::Bool(theme.reduced_motion)),
    ])
}

fn zone(zone: Zone) -> Value {
    let name = match zone {
        Zone::Row => "row",
        Zone::Actions => "actions",
        Zone::Tabs => "tabs",
        Zone::Other => "other",
    };
    Value::EnumerationValue("LayoutZone".to_owned(), name.to_owned())
}

fn record<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Struct(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect::<Struct>(),
    )
}
