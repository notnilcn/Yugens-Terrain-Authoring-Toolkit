pub fn space_enum_hint(p: &mut godot::register::info::PropertyInfo) {
    if p.hint_info.hint != godot::register::info::PropertyHint::ENUM {
        return;
    }
    let src = p.hint_info.hint_string.to_string();
    let spaced = space_camel_list(&src);
    if spaced != src {
        p.hint_info.hint_string = spaced.as_str().into();
    }
}
fn space_camel_list(src: &str) -> String {
    let mut out = String::with_capacity(src.len() + 8);
    let mut prev: Option<char> = None;
    for c in src.chars() {
        if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase()) {
            out.push(' ');
        }
        out.push(c);
        prev = Some(c);
    }
    out
}
#[cfg(test)]
mod tests {
    #[test]
    fn spaces_camel_case_enum_items() {
        assert_eq!(
            super::space_camel_list("Square,Triangle,Hex,HexRings"),
            "Square,Triangle,Hex,Hex Rings"
        );
        assert_eq!(
            super::space_camel_list("Anchor:0,LowRes:1"),
            "Anchor:0,Low Res:1"
        );
    }
}
