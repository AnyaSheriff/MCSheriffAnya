pub(crate) fn split_top_level(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut start = 0;

    for (at, c) in text.char_indices() {
        match (quote, c) {
            (Some(_), _) if escaped => escaped = false,
            (Some(_), '\\') => escaped = true,
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '[' | '{') => depth += 1,
            (None, ']' | '}') => depth -= 1,
            (None, ',') if depth == 0 => {
                parts.push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }

    if !text[start..].trim().is_empty() {
        parts.push(&text[start..]);
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::split_top_level;

    fn check(input: &str, expected: &[&str]) {
        assert_eq!(split_top_level(input), expected, "input: {input:?}");
    }

    #[test]
    fn double_escaped_quote_then_comma() {
        check(r#"name="alpha\",beta",damage=7"#, &[r#"name="alpha\",beta""#, "damage=7"]);
    }

    #[test]
    fn single_escaped_quote_then_comma() {
        check(r#"name='alpha\',beta',damage=7"#, &[r#"name='alpha\',beta'"#, "damage=7"]);
    }

    #[test]
    fn even_backslashes_before_closing_quote() {
        check(r#"a="c\\",b=2"#, &[r#"a="c\\""#, "b=2"]);
    }

    #[test]
    fn odd_backslash_parity_overrides_comma() {
        check(r#"a="x\",b",c=3"#, &[r#"a="x\",b""#, "c=3"]);
    }

    #[test]
    fn commas_and_braces_within_strings() {
        check("a=\"[1,{2},3]\",b=\"{,}\"", &[r#"a="[1,{2},3]""#, r#"b="{,}""#]);
    }

    #[test]
    fn normal_nested_multiple_components() {
        check("a=[1,2],b={c:3},d=4", &["a=[1,2]", "b={c:3}", "d=4"]);
    }

    #[test]
    fn unicode_bytes_and_slices_exact() {
        check("ключ=\"знач,ение\",флаг=✓", &[r#"ключ="знач,ение""#, "флаг=✓"]);
    }

    #[test]
    fn trailing_empty_part_omitted() {
        check("a=1,b=2,", &["a=1", "b=2"]);
    }

    #[test]
    fn raw_spec_example_rust_strings() {
        check(
            r#"custom_name="alpha\",beta",damage=7"#,
            &[r#"custom_name="alpha\",beta""#, "damage=7"],
        );
    }
}
