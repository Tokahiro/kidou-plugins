//! The check `kidouplug` runs on a layout before it installs it, and again after each update of
//! Kidō: Slint's own errors, then the rules that keep Home at rest and in charge of the focus,
//! the keys and the windows.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use i_slint_compiler::CompilerConfiguration;
use i_slint_compiler::diagnostics::{BuildDiagnostics, DiagnosticLevel};
use i_slint_compiler::element_type::ElementType;
use i_slint_compiler::expression_tree::{Callable, Expression};
use i_slint_compiler::expression_types::{ImageReference, Unit};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::langtype::Type;
use i_slint_compiler::object_tree::{self, Component, ElementRc, PropertyAnimation};
use i_slint_compiler::source_path::SourcePath;

use crate::{Files, ROOT, block_on, source};

/// The longest an animation runs once started, its delay and every iteration counted, in
/// milliseconds: Home's own longest motion is under a second.
pub const ANIMATION_MAX_MS: f64 = 3000.0;

/// The most pixels all images of a layout decode to together, each counted as often as it is
/// shown: one image of [`IMAGE_SIDE_MAX`](crate::IMAGE_SIDE_MAX) at each side, 64 MiB.
pub const PIXELS_MAX: u64 = 1 << 24;

/// The most iterations an animation may run.
const ITERATIONS_MAX: f64 = 10.0;

/// Elements a layout may not use: the shell keeps the focus, the keys and the windows, and no
/// timer runs while Home rests.
const REFUSED_ELEMENTS: &[&str] = &[
    "FocusScope",
    "TextInput",
    "PopupWindow",
    "ContextMenuArea",
    "ContextMenu",
    "MenuBar",
    "SystemTrayIcon",
    "Timer",
    "Dialog",
    "SwipeGestureHandler",
];

/// Why the layout `files` is refused, if it is: Slint's errors, then each rule it breaks.
pub fn check(files: &Files) -> Result<(), Vec<String>> {
    let Some(text) = files.get(ROOT) else {
        return Err(vec![format!("{ROOT} is missing")]);
    };
    let text =
        String::from_utf8(text.to_vec()).map_err(|_| vec![format!("{ROOT}: no UTF-8 text")])?;
    let images = Rc::new(RefCell::new(source::Images::default()));
    let mut config = CompilerConfiguration::new(OutputFormat::Interpreter);
    source::configure(&mut config, Rc::new(files.clone()), images.clone());
    let mut diagnostics = BuildDiagnostics::default();
    let syntax =
        i_slint_compiler::parser::parse(text, Some(source::address(ROOT)), &mut diagnostics);
    let (document, diagnostics, _) = block_on(i_slint_compiler::compile_syntax_node(
        syntax,
        diagnostics,
        config,
    ));
    if diagnostics.has_errors() {
        return Err(diagnostics
            .iter()
            .filter(|found| found.level() == DiagnosticLevel::Error)
            .map(|found| format!("slint: {}", found.message()))
            .collect());
    }
    let images = images.take();
    let mut why = images.refused;
    let mut known = HashMap::new();
    let globals = document.used_types.borrow().globals.clone();
    let pixels = document
        .exported_roots()
        .chain(globals)
        .fold(0u64, |pixels, root| {
            pixels.saturating_add(image_rules(&root, &images.mapped, &mut known, &mut why).0)
        });
    if pixels > PIXELS_MAX {
        why.push(format!(
            "its images decode to {pixels} pixels as often as they are shown, more than \
             {PIXELS_MAX}"
        ));
    }
    if !document.custom_fonts.is_empty() {
        why.push("a font: a layout uses Kidō's fonts".to_owned());
    }
    let roots: Vec<ElementRc> = document
        .exported_roots()
        .map(|root| root.root_element.clone())
        .collect();
    let mut nested = HashMap::new();
    document.visit_all_used_components(|component| {
        let name = &component.id;
        call_rules(name, component, &roots, &mut why);
        if !component.timers.borrow().is_empty() {
            why.push(format!("{name}: a Timer"));
        }
        object_tree::recurse_elem_including_sub_components(component, &(), &mut |element, ()| {
            element_rules(name, element, &mut nested, &mut why);
        });
        object_tree::visit_all_expressions(component, |expression, _| {
            expression.visit_recursive(&mut |expression| {
                if let Expression::FunctionCall {
                    function: Callable::Builtin(function),
                    ..
                } = expression
                {
                    let called = format!("{function:?}");
                    let called = called.split('(').next().unwrap_or_default();
                    if !function.is_pure() || called == "AnimationTick" {
                        why.push(format!("{name}: a call of {called}"));
                    }
                }
            });
        });
    });
    why.sort();
    why.dedup();
    if why.is_empty() { Ok(()) } else { Err(why) }
}

/// The root's callbacks a layout calls: each only in a TouchArea's `clicked`.
const TAPS: [&str; 2] = ["activated", "tab-tapped"];

/// The callback calls of `component`, its `init` code counted: a layout calls no callback but the
/// root's [`TAPS`], and those only in a TouchArea's `clicked`, so that Home acts on a tap alone.
fn call_rules(name: &str, component: &Rc<Component>, roots: &[ElementRc], why: &mut Vec<String>) {
    let mut refuse = |expression: &Expression, clicked: bool| {
        expression.visit_recursive(&mut |expression| {
            let Expression::FunctionCall {
                function: Callable::Callback(callback),
                ..
            } = expression
            else {
                return;
            };
            let tap = TAPS.contains(&callback.name().as_str())
                && roots
                    .iter()
                    .any(|root| Rc::ptr_eq(root, &callback.element()));
            if !(tap && clicked) {
                why.push(format!(
                    "{name}: a call of {} outside a TouchArea's `clicked`: a layout calls only \
                     {}, and only on a tap",
                    callback.name(),
                    TAPS.join(" and ")
                ));
            }
        });
    };
    let mut owners = HashSet::new();
    let mut visit = |element: &ElementRc| {
        let touch = matches!(
            &element.borrow().base_type,
            ElementType::Builtin(builtin) if builtin.name == "TouchArea"
        );
        object_tree::visit_element_expressions(element, |expression, property, _| {
            refuse(expression, touch && property == Some("clicked"));
        });
        let owner = element.borrow().enclosing_component.upgrade();
        if let Some(owner) = owner
            && owners.insert(Rc::as_ptr(&owner))
        {
            let init = owner.init_code.borrow();
            for code in init
                .constructor_code
                .iter()
                .chain(init.inlined_init_code.values())
            {
                refuse(code, false);
            }
        }
    };
    for element in component.optimized_elements.borrow().iter() {
        visit(element);
    }
    object_tree::recurse_elem_including_sub_components(component, &(), &mut |element, ()| {
        visit(element);
    });
}

/// The rules of one element of the component `name`: its kind, its repeat, its `changed`
/// handlers and its animations.
fn element_rules(
    name: &str,
    element: &ElementRc,
    nested: &mut HashMap<*const Component, bool>,
    why: &mut Vec<String>,
) {
    let element = element.borrow();
    if let ElementType::Builtin(builtin) = &element.base_type
        && REFUSED_ELEMENTS.contains(&builtin.name.as_str())
    {
        why.push(format!("{name}: a {}", builtin.name));
    }
    if let Some(repeated) = element
        .repeated
        .as_ref()
        .filter(|r| !r.is_conditional_element)
    {
        let mut model = &repeated.model;
        while let Expression::Cast { from, .. } = model {
            model = from;
        }
        if matches!(model.ty(), Type::Int32 | Type::Float32) {
            why.push(format!("{name}: a `for` over a number in {}", element.id));
        }
        if let ElementType::Component(body) = &element.base_type
            && repeats(body, nested)
        {
            why.push(format!("{name}: a `for` inside a `for` in {}", element.id));
        }
    }
    if !element.change_callbacks.is_empty() {
        why.push(format!("{name}: a `changed` handler on {}", element.id));
    }
    for (property, binding) in element.bindings_including_synthetic() {
        let animations = match &binding.borrow().animation {
            Some(PropertyAnimation::Static(animation)) => vec![animation.clone()],
            Some(PropertyAnimation::Transition { animations, .. }) => animations
                .iter()
                .map(|transition| transition.animation.clone())
                .collect(),
            None => Vec::new(),
        };
        for animation in animations {
            if let Some(refusal) = animation_refusal(&animation) {
                why.push(format!("{name}: the animation of {property} {refusal}"));
            }
        }
    }
}

/// The pixels the images of `element`, in the component `name`, decode to, and whether one is
/// of the layout's own, by the image rules: each image is one of the layout's own that the
/// resource mapper turned into an address of `mapped`, or one of Slint's own library; an image of
/// the layout's own is the value of a property, never of a function, a callback, `init`, a
/// `changed` handler or the model of a `for`, as each call decodes its own copy. An image of
/// Slint's library is one only where Slint's library names it: Slint draws its SVGs at the size
/// of their element.
fn image_sites(
    name: &str,
    element: &ElementRc,
    mapped: &HashMap<String, u64>,
    why: &mut Vec<String>,
) -> (u64, bool) {
    let (mut pixels, mut own) = (0u64, false);
    object_tree::visit_element_expressions(element, |expression, property, ty| {
        let held = property.is_some()
            && !matches!(
                ty(),
                Type::Function(_) | Type::Callback(_) | Type::Void | Type::Invalid
            );
        expression.visit_recursive(&mut |expression| {
            let Expression::ImageReference {
                resource_ref,
                source_location,
                ..
            } = expression
            else {
                return;
            };
            let library = source_location
                .as_ref()
                .and_then(|at| at.source_file.as_ref())
                .is_some_and(|file| matches!(file.path(), SourcePath::Builtin(_)));
            match resource_ref {
                ImageReference::None => {}
                ImageReference::Source(SourcePath::Builtin(_)) if library => {}
                ImageReference::DataUri(data) if data == source::NO_IMAGE => {}
                ImageReference::DataUri(data) => match mapped.get(data.as_str()) {
                    Some(&each) if held => {
                        pixels = pixels.saturating_add(each);
                        own = true;
                    }
                    Some(_) => why.push(format!(
                        "{name}: an image of the layout's own in a function, a callback, \
                         `init`, `changed` or a `for`'s model: a layout gives its images to \
                         properties"
                    )),
                    None => why.push(format!(
                        "{name}: an image written as a `data:` address: a layout names its \
                         images by their files"
                    )),
                },
                ImageReference::Source(SourcePath::Builtin(_)) => why.push(format!(
                    "{name}: an image of Slint's library: a layout shows its own images"
                )),
                _ => why.push(format!("{name}: an image that is not a file of the layout")),
            }
        });
    });
    (pixels, own)
}

/// The pixels one `component` decodes its images to, each component it holds counted as often
/// as it is shown, and whether it shows an image of the layout's own; no image of the layout's
/// own is inside a `for`, as each repeat decodes its own copy. From the exported roots and the
/// globals it reaches every component the layout shows but a popup's, which the check refuses.
fn image_rules(
    component: &Rc<Component>,
    mapped: &HashMap<String, u64>,
    known: &mut HashMap<*const Component, (u64, bool)>,
    why: &mut Vec<String>,
) -> (u64, bool) {
    let key = Rc::as_ptr(component);
    if let Some(&found) = known.get(&key) {
        return found;
    }
    let name = &component.id;
    let (mut pixels, mut own) = (0u64, false);
    let mut count = |element: &ElementRc| {
        let (more, shows) = image_sites(name, element, mapped, why);
        pixels = pixels.saturating_add(more);
        own |= shows;
    };
    let mut inner = Vec::new();
    for element in component.optimized_elements.borrow().iter() {
        count(element);
    }
    object_tree::recurse_elem(&component.root_element, &(), &mut |element, ()| {
        count(element);
        let element = element.borrow();
        if let ElementType::Component(base) = &element.base_type {
            let repeat = element.repeated.as_ref();
            inner.push((
                base.clone(),
                repeat.is_some_and(|r| !r.is_conditional_element),
                element.id.clone(),
            ));
        }
    });
    for (base, repeated, id) in inner {
        let (more, shows) = image_rules(&base, mapped, known, why);
        if repeated && shows {
            why.push(format!(
                "{name}: an image of the layout's own inside a `for` in {id}"
            ));
        }
        pixels = pixels.saturating_add(more);
        own |= shows;
    }
    known.insert(key, (pixels, own));
    (pixels, own)
}

/// Whether `component`, or a component it holds, repeats an element with `for`.
fn repeats(component: &Rc<Component>, known: &mut HashMap<*const Component, bool>) -> bool {
    let key = Rc::as_ptr(component);
    if let Some(&found) = known.get(&key) {
        return found;
    }
    known.insert(key, false);
    let mut found = false;
    object_tree::recurse_elem(&component.root_element, &(), &mut |element, ()| {
        let element = element.borrow();
        found |= element
            .repeated
            .as_ref()
            .is_some_and(|repeated| !repeated.is_conditional_element);
        if let ElementType::Component(inner) = &element.base_type {
            found |= repeats(inner, known);
        }
    });
    known.insert(key, found);
    found
}

/// Why the animation `animation` is refused, if it is: its iterations, its duration and its
/// delay are not numbers written in the file, or it runs longer than [`ANIMATION_MAX_MS`].
fn animation_refusal(animation: &ElementRc) -> Option<&'static str> {
    let animation = animation.borrow();
    let literal = |name: &str, unit: Unit, absent: f64| -> Option<f64> {
        let Some(binding) = animation.binding(name) else {
            return Some(absent);
        };
        let mut expression = &binding.expression;
        while let Expression::Cast { from, .. } = expression {
            expression = from;
        }
        match expression {
            Expression::NumberLiteral(value, found) if *found == unit && *value >= 0.0 => {
                Some(*value)
            }
            _ => None,
        }
    };
    let Some(iterations) = literal("iteration-count", Unit::None, 1.0) else {
        return Some("has no iteration-count written as a number from 0 up");
    };
    if iterations > ITERATIONS_MAX {
        return Some("runs more than 10 times");
    }
    let (Some(duration), Some(delay)) = (
        literal("duration", Unit::Ms, 0.0),
        literal("delay", Unit::Ms, 0.0),
    ) else {
        return Some("has a duration or a delay not written as a time");
    };
    (duration * iterations + delay > ANIMATION_MAX_MS).then_some("runs longer than 3 seconds")
}
