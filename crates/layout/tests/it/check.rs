use kidou_layout::{Files, check, compile};

use crate::{good, layout};

/// PNGs of 5000 by 10 pixels, of 64 by 64, of 4096 by 4096 and an animated one of 64 by 64: the
/// chunks the check reads, with no check sums or pixels.
const BIG: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\x13\x88\0\0\0\x0a\x08\x06\0\0\0crc!\0\0\0\0IDATcrc!";
const SMALL: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x40\0\0\0\x40\x08\x06\0\0\0crc!\0\0\0\0IDATcrc!";
const FULL: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\x10\0\0\0\x10\0\x08\x06\0\0\0crc!\0\0\0\0IDATcrc!";
const ANIMATED: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR\0\0\0\x40\0\0\0\x40\x08\x06\0\0\0crc!\0\0\0\x08acTL\0\0\0\x02\0\0\0\0crc!\0\0\0\0IDATcrc!";
/// An SVG of 64 by 64 and one of 30 000 by 30 000, which the check draws at twice their size,
/// at most 4096 at a side.
const SMALL_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><rect width='64' height='64'/></svg>";
const BIG_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' width='30000' height='30000'><rect width='30000' height='30000'/></svg>";
/// A JPEG whose frame header of 30 000 by 30 000 pixels follows a fill byte, and whose comment
/// holds a frame header of 64 by 64, which a reader that takes the fill byte for a marker finds.
const FILLED: &[u8] = b"\xff\xd8\xff\xff\xc0\0\x11\x08\x75\x30\x75\x30\x03\x01\x11\0\x02\x11\0\x03\x11\0\xff\xfe\0\x0b\xff\xc0\0\x11\x08\0\x40\0\x40\xff\xda\0\x02";

/// A layout written to misbehave: its name, its root's body, the files beside it, and what the
/// check's reason names.
struct Case {
    name: &'static str,
    body: &'static str,
    extra: &'static [(&'static str, &'static [u8])],
    expected: &'static str,
}

#[test]
fn the_sample_passes_the_check_and_compiles_under_homes_root() {
    let files = crate::drawn(Files::read(&good()).expect("the sample reads"));
    assert_eq!(check(&files), Ok(()));
    let definition = compile(&files).expect("the sample compiles");
    let names: Vec<String> = definition.properties().map(|(name, _)| name).collect();
    let root = [
        "tiles",
        "focused",
        "zone",
        "stage",
        "hero",
        "empty",
        "empty-words",
        "tabs",
        "tab",
        "theme",
    ];
    for name in root {
        assert!(names.contains(&name.to_owned()), "{name} in {names:?}");
    }
    assert!(definition.callbacks().any(|name| name == "activated"));
}

#[test]
fn a_layout_without_the_contracts_properties_does_not_compile() {
    let files = Files::of([(
        kidou_layout::ROOT,
        b"export component Layout { in property <int> focused; }".as_slice(),
    )])
    .expect("the test's files");
    assert_eq!(check(&files), Ok(()));
    assert!(compile(&files).is_err());
}

const CASES: &[Case] = &[
    Case {
        name: "a timer",
        body: "Timer { interval: 16ms; triggered => {} }",
        extra: &[],
        expected: "Timer",
    },
    Case {
        name: "an endless animation",
        body: "property <float> v: root.focused; animate v { duration: 1s; iteration-count: -1; } Rectangle { opacity: v; }",
        extra: &[],
        expected: "iteration-count",
    },
    Case {
        name: "an iteration count from a property",
        body: "property <float> n: root.focused; property <float> v: root.focused; animate v { duration: 1s; iteration-count: n; } Rectangle { opacity: v; }",
        extra: &[],
        expected: "iteration-count",
    },
    Case {
        name: "an animation of 24 hours",
        body: "property <float> v: root.focused; animate v { duration: 86400s; } Rectangle { opacity: v; }",
        extra: &[],
        expected: "longer than",
    },
    Case {
        name: "a delay from a property",
        body: "property <duration> d: root.focused * 1ms; property <float> v: root.focused; animate v { delay: d; } Rectangle { opacity: v; }",
        extra: &[],
        expected: "duration or a delay",
    },
    Case {
        name: "animation-tick",
        body: "Rectangle { x: animation-tick() / 1ms * 1px; }",
        extra: &[],
        expected: "AnimationTick",
    },
    Case {
        name: "a changed handler",
        body: "property <int> n; changed n => { }",
        extra: &[],
        expected: "changed",
    },
    Case {
        name: "a focus scope",
        body: "FocusScope { }",
        extra: &[],
        expected: "FocusScope",
    },
    Case {
        name: "a popup",
        body: "p := PopupWindow { } init => { p.show(); }",
        extra: &[],
        expected: "PopupWindow",
    },
    Case {
        name: "an open link",
        body: "TouchArea { clicked => { Platform.open-url(\"https://example.com\"); } }",
        extra: &[],
        expected: "OpenUrl",
    },
    Case {
        name: "a push onto an array",
        body: "property <[int]> list; init => { list.push(1); }",
        extra: &[],
        expected: "ArrayPush",
    },
    Case {
        name: "a call of activated in init, with no tap",
        body: "for t[i] in root.tiles: Rectangle { init => { root.activated(i); } }",
        extra: &[],
        expected: "activated outside",
    },
    Case {
        name: "a call of activated in the init of an if, as the focus moves",
        body: "if root.focused == 2: Rectangle { init => { root.activated(5); } }",
        extra: &[],
        expected: "activated outside",
    },
    Case {
        name: "a call of activated in the root's init",
        body: "init => { root.activated(root.focused); }",
        extra: &[],
        expected: "activated outside",
    },
    Case {
        name: "a call of tab-tapped as the tab changes, with no tap",
        body: "changed tab => { root.tab-tapped(root.tab + 1); }",
        extra: &[],
        expected: "tab-tapped outside",
    },
    Case {
        name: "a call of activated on a pointer event, not a click",
        body: "TouchArea { pointer-event(e) => { root.activated(1); } }",
        extra: &[],
        expected: "activated outside",
    },
    Case {
        name: "a call of activated in a function a click calls",
        body: "function go() { root.activated(1); } TouchArea { clicked => { go(); } }",
        extra: &[],
        expected: "activated outside",
    },
    Case {
        name: "a TouchArea's clicked called with no tap",
        body: "ta := TouchArea { clicked => { root.activated(1); } } init => { ta.clicked(); }",
        extra: &[],
        expected: "clicked outside",
    },
    Case {
        name: "a for over a number",
        body: "for i in 1000000: Rectangle { }",
        extra: &[],
        expected: "over a number",
    },
    Case {
        name: "a for inside a for",
        body: "for a in root.tiles: Rectangle { for b in root.tiles: Rectangle { } }",
        extra: &[],
        expected: "inside a `for`",
    },
    Case {
        name: "an image by an absolute path",
        body: "Image { source: @image-url(\"/etc/hostname.png\"); }",
        extra: &[],
        expected: "not in the layout",
    },
    Case {
        name: "an image above the folder",
        body: "Image { source: @image-url(\"../mark.png\"); }",
        extra: &[],
        expected: "not in the layout",
    },
    Case {
        name: "a large image",
        body: "Image { source: @image-url(\"big.png\"); }",
        extra: &[("big.png", BIG)],
        expected: "larger than 4096",
    },
    Case {
        name: "a kind of image Kidō does not read",
        body: "Image { source: @image-url(\"a.gif\"); }",
        extra: &[("a.gif", SMALL)],
        expected: "a PNG, a JPEG or an SVG",
    },
    Case {
        name: "an image written as a data: address",
        body: "Image { source: @image-url(\"data:image/png;base64,iVBORw0KGgo=\"); }",
        extra: &[],
        expected: "`data:` address",
    },
    Case {
        name: "an SVG written as a data: address that links a file",
        body: "Image { source: @image-url(\"data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><image href='/etc/x.png'/></svg>\"); }",
        extra: &[],
        expected: "`data:` address",
    },
    Case {
        name: "an animated PNG",
        body: "Image { source: @image-url(\"spin.png\"); }",
        extra: &[("spin.png", ANIMATED)],
        expected: "an animated PNG",
    },
    Case {
        name: "a large JPEG behind a fill byte",
        body: "Image { source: @image-url(\"big.jpg\"); }",
        extra: &[("big.jpg", FILLED)],
        expected: "larger than 4096",
    },
    Case {
        name: "a large SVG shown twice, each drawn at 4096 by 4096",
        body: "Image { source: @image-url(\"big.svg\"); } Image { source: @image-url(\"big.svg\"); }",
        extra: &[("big.svg", BIG_SVG)],
        expected: "pixels",
    },
    Case {
        name: "an image of the layout's own inside a for",
        body: "for t in root.tiles: Image { source: @image-url(\"a.png\"); }",
        extra: &[("a.png", SMALL)],
        expected: "inside a `for`",
    },
    Case {
        name: "images that decode to more pixels than the cap",
        body: "Image { source: @image-url(\"full.png\"); } Image { source: @image-url(\"full.png\"); }",
        extra: &[("full.png", FULL)],
        expected: "pixels",
    },
    Case {
        name: "an image of the layout's own from a function called in a for",
        body: "pure function pic(i: int) -> image { return i >= 0 ? @image-url(\"a.png\") : @image-url(\"a.png\"); } \
               for t[i] in root.tiles: Image { source: root.pic(i); }",
        extra: &[("a.png", SMALL)],
        expected: "in a function",
    },
    Case {
        name: "an image of the layout's own from a callback",
        body: "pure callback pic(int) -> image; pic(i) => { return @image-url(\"a.png\"); } \
               Image { source: root.pic(root.focused); }",
        extra: &[("a.png", SMALL)],
        expected: "in a function",
    },
    Case {
        name: "an image of the layout's own in a for's model",
        body: "for p in [@image-url(\"a.png\")]: Image { source: p; }",
        extra: &[("a.png", SMALL)],
        expected: "a `for`'s model",
    },
    Case {
        name: "an SVG of Slint's library, drawn at any size",
        body: "for t in root.tiles: Image { source: @image-url(\"builtin:/fluent/_arrow-down.svg\"); width: 30000px; height: 30000px; }",
        extra: &[],
        expected: "Slint's library",
    },
    Case {
        name: "an image of the layout's own set in init",
        body: "property <image> pic; init => { self.pic = @image-url(\"a.png\"); } \
               Image { source: root.pic; }",
        extra: &[("a.png", SMALL)],
        expected: "in a function",
    },
];

#[test]
fn the_check_refuses_what_keeps_home_awake_or_takes_its_focus_keys_or_files() {
    for Case {
        name,
        body,
        extra,
        expected,
    } in CASES
    {
        let refused = check(&layout(body, extra)).expect_err(name);
        assert!(
            refused.iter().any(|why| why.contains(expected)),
            "{name}: {refused:?}"
        );
    }
}

#[test]
fn a_layout_reaches_no_slint_file_outside_its_copy() {
    let outside = crate::folder("outside").join("other.slint");
    std::fs::write(&outside, "export component Other { }").expect("the file outside");
    let imports = [
        format!("import {{ Other }} from \"{}\";", outside.display()),
        "import { Other } from \"../other.slint\";".to_owned(),
        "import { Other } from \"https://example.com/other.slint\";".to_owned(),
        "import { Other } from \"missing.slint\";".to_owned(),
    ];
    for import in imports {
        let text = format!("{import}\nexport component Layout {{ Other {{ }} }}");
        let files = Files::of([(kidou_layout::ROOT, text.as_bytes())]).expect("the test's files");
        let refused = check(&files).expect_err(&import);
        assert!(
            refused.iter().any(|why| why.starts_with("slint:")),
            "{import}: {refused:?}"
        );
        assert!(compile(&files).is_err(), "{import}");
    }
}

#[test]
fn a_layout_uses_kidos_fonts() {
    let files = layout(
        "Text { text: \"x\"; }",
        &[("font.ttf", b"no font".as_slice())],
    );
    assert_eq!(check(&files), Ok(()));
    let text = "import \"font.ttf\";\nexport component Layout { }";
    let mut entries = vec![(kidou_layout::ROOT, text.as_bytes())];
    entries.push(("font.ttf", b"no font".as_slice()));
    let refused = check(&Files::of(entries).expect("the test's files")).expect_err("a font");
    assert!(
        refused.iter().any(|why| why.contains("font")),
        "{refused:?}"
    );
}

#[test]
fn an_image_shown_once_or_under_an_if_passes() {
    let body = "property <image> pic: root.focused > 0 ? @image-url(\"b.png\") : @image-url(\"\"); \
                Image { source: root.pic; } \
                if root.focused > 0: Image { source: @image-url(\"a.png\"); }";
    let files = layout(body, &[("a.png", SMALL), ("b.png", SMALL)]);
    assert_eq!(check(&files), Ok(()));
}

#[test]
fn an_svg_passes_as_the_png_drawn_of_it_whatever_its_elements_size() {
    let body = "Image { source: @image-url(\"a.svg\"); width: 30000px; height: 30000px; }";
    let files = layout(body, &[("a.svg", SMALL_SVG)]);
    assert_eq!(check(&files), Ok(()));
    let files = layout(
        "Image { source: @image-url(\"big.svg\"); }",
        &[("big.svg", BIG_SVG)],
    );
    assert_eq!(check(&files), Ok(()));
    // 128 by 128 and 4096 by 4096 pixels together pass the cap.
    let body =
        "Image { source: @image-url(\"a.svg\"); } Image { source: @image-url(\"big.svg\"); }";
    let files = layout(body, &[("a.svg", SMALL_SVG), ("big.svg", BIG_SVG)]);
    let refused = check(&files).expect_err("over the cap");
    assert!(
        refused.iter().any(|why| why.contains("pixels")),
        "{refused:?}"
    );
}

#[test]
fn an_svg_not_drawn_is_refused() {
    let text = "export component Layout { Image { source: @image-url(\"a.svg\"); } }";
    let files = Files::of([(kidou_layout::ROOT, text.as_bytes()), ("a.svg", SMALL_SVG)])
        .expect("the test's files");
    let refused = check(&files).expect_err("not drawn");
    assert!(
        refused.iter().any(|why| why.contains("not drawn")),
        "{refused:?}"
    );
}

#[test]
fn drawing_refuses_an_svg_that_reads_a_file_or_holds_a_dtd() {
    let cases: [(&str, &[u8], &str); 4] = [
        (
            "a link to a file",
            b"<svg xmlns='http://www.w3.org/2000/svg'><linearGradient id='g' href='/etc/x.svg'/></svg>",
            "links",
        ),
        (
            "a link with a space before its '#'",
            b"<svg xmlns='http://www.w3.org/2000/svg'><linearGradient id='g' href=' #g'/></svg>",
            "links",
        ),
        (
            "a DTD",
            b"<!DOCTYPE svg [<!ENTITY x SYSTEM '/etc/passwd'>]><svg xmlns='http://www.w3.org/2000/svg'/>",
            "a DTD",
        ),
        (
            "an image behind a '#'",
            b"<svg xmlns='http://www.w3.org/2000/svg'><image width='9' height='9' href='#/../x.png'/></svg>",
            "an image",
        ),
    ];
    for (name, svg, expected) in cases {
        let refused = kidou_layout::draw_svg(svg).expect_err(name);
        assert!(refused.contains(expected), "{name}: {refused}");
    }
    let filter = b"<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><filter id='f'><feGaussianBlur stdDeviation='2'/></filter><rect width='64' height='64' filter='url(#f)'/></svg>";
    assert!(kidou_layout::draw_svg(filter).is_ok(), "a filter draws");
}

#[test]
fn a_global_holds_its_images_to_the_same_rules() {
    const LINKS: &str = "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><image href='/etc/x.png'/></svg>";
    let cases = [
        (
            format!(
                "global G {{ public pure function pic(n: int) -> image {{ \
                 return n > 9 ? @image-url(\"{LINKS}\") : @image-url(\"data:,\"); }} }}"
            ),
            "Image { source: G.pic(root.focused); }",
            "`data:` address",
        ),
        (
            format!("export global G {{ out property <image> pic: @image-url(\"{LINKS}\"); }}"),
            "Image { source: G.pic; }",
            "`data:` address",
        ),
        (
            "global G { public pure function pic(n: int) -> image { \
             return n > 9 ? @image-url(\"full.png\") : @image-url(\"full.png\"); } }"
                .to_owned(),
            "Image { source: G.pic(root.focused); }",
            "in a function",
        ),
        (
            "global G { out property <image> pic: @image-url(\"full.png\"); }".to_owned(),
            "Image { source: G.pic; } Image { source: @image-url(\"full.png\"); }",
            "pixels",
        ),
    ];
    for (global, body, expected) in cases {
        let text =
            format!("{global}\nexport component Layout {{ in property <int> focused; {body} }}");
        let files = Files::of([(kidou_layout::ROOT, text.as_bytes()), ("full.png", FULL)])
            .expect("the test's files");
        let refused = check(&files).expect_err(&text);
        assert!(
            refused.iter().any(|why| why.contains(expected)),
            "{text}: {refused:?}"
        );
    }
}

#[test]
fn an_image_counts_once_for_each_time_its_component_is_shown() {
    let text = "component Big inherits Image { source: @image-url(\"full.png\"); }\n\
                export component Layout { Big { } Big { } }";
    let files = Files::of([(kidou_layout::ROOT, text.as_bytes()), ("full.png", FULL)])
        .expect("the test's files");
    let refused = check(&files).expect_err("two full images");
    assert!(
        refused.iter().any(|why| why.contains("pixels")),
        "{refused:?}"
    );
}
