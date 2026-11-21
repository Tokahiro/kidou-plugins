use super::{first_arguments, template};

fn pairs(fields: &[(&str, &str)]) -> Vec<(String, String)> {
    (fields.iter())
        .map(|&(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

#[test]
fn a_template_s_values_are_read_past_inner_templates_and_links() {
    let page = "{{Video game|hdr = true}}\n\
                {{Video\n\
                |HDR = false\n\
                |120 fps notes = Capped at 600.{{Refcheck|user=X|date=2024}}\n\
                |ultrawidescreen notes = See [[#Ultra|Ultra-widescreen]].\n\
                |fov =\n\
                }}\n\
                {{Video\n|hdr = true\n}}";
    assert_eq!(
        template(page, "Video"),
        pairs(&[
            ("hdr", "false"),
            (
                "120 fps notes",
                "Capped at 600.{{Refcheck|user=X|date=2024}}"
            ),
            ("ultrawidescreen notes", "See [[#Ultra|Ultra-widescreen]]."),
            ("fov", ""),
        ])
    );
}

#[test]
fn no_template_or_an_unclosed_one_has_no_values() {
    assert_eq!(template("{{Input\n|hdr = true\n}}", "Video"), Vec::new());
    assert_eq!(template("{{Video\n|hdr = true\n", "Video"), Vec::new());
    assert_eq!(template("{{Videos|hdr = true}}", "Video"), Vec::new());
    assert_eq!(template("", "Video"), Vec::new());
}

#[test]
fn the_first_argument_of_each_row_is_read() {
    let page = "{{Infobox game\n|engines =\n\
                {{Infobox game/row/engine|Unreal Engine 4|ref=<ref>{{Refurl|url=x}}</ref>}}\n\
                {{Infobox game/row/engine| [[Engine:Wwise|Wwise]] |Audio}}\n\
                {{Infobox game/row/engine||}}\n\
                {{Infobox game/row/engine|Unclosed";
    assert_eq!(
        first_arguments(page, "Infobox game/row/engine"),
        ["Unreal Engine 4", "[[Engine:Wwise|Wwise]]"]
    );
    assert_eq!(
        first_arguments("", "Infobox game/row/engine"),
        Vec::<String>::new()
    );
}
