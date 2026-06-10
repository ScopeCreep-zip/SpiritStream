//! JSONC sanitisation shared by the theme loaders.
//!
//! The bundled theme files (`themes/*.jsonc`) and the compile-time
//! embedded copies are authored as JSONC: they carry `//` / `/* */`
//! comments and trailing commas. `serde_json` accepts neither, so both
//! the filesystem loader ([`super::theme_manager`]) and the embedded
//! fallback ([`super::embedded_themes`]) run their input through
//! [`sanitize_jsonc`] before parsing.
//!
//! This lives in one place on purpose: a divergence between the two
//! sanitisers is exactly what let every theme silently fail to parse —
//! comments were stripped but trailing commas were not, so `serde_json`
//! rejected every file and the embedded accessibility fallback returned
//! an empty set.

/// Strip a leading UTF-8 BOM plus `//` line and `/* */` block comments.
/// String literals are preserved verbatim so a `//` inside a token value
/// (e.g. a URL) is never treated as a comment.
pub(crate) fn strip_jsonc_comments(input: &str) -> String {
    let input = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                output.push(ch);
            }
            continue;
        }

        if in_block_comment {
            if ch == '*' {
                if let Some('/') = chars.peek() {
                    chars.next();
                    in_block_comment = false;
                }
                continue;
            }
            if ch == '\n' {
                output.push(ch);
            }
            continue;
        }

        if in_string {
            output.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.next();
                    in_line_comment = true;
                    continue;
                }
                Some('*') => {
                    chars.next();
                    in_block_comment = true;
                    continue;
                }
                _ => {}
            }
        }

        output.push(ch);
    }

    output
}

/// Remove trailing commas: a `,` whose next non-whitespace character is
/// `}` or `]`. String-aware so a comma inside a token value is never
/// touched. Run after [`strip_jsonc_comments`] so no comment can sit
/// between the comma and the closing brace.
pub(crate) fn strip_trailing_commas(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        if ch == ',' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && (chars[j] == '}' || chars[j] == ']') {
                // Drop the comma; the following whitespace + brace are
                // emitted on subsequent iterations.
                i += 1;
                continue;
            }
        }

        out.push(ch);
        i += 1;
    }

    out
}

/// Full JSONC → JSON sanitisation: drop the BOM, strip comments, then
/// strip trailing commas. The result is plain JSON `serde_json` accepts.
pub(crate) fn sanitize_jsonc(input: &str) -> String {
    strip_trailing_commas(&strip_jsonc_comments(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_and_block_comments() {
        let input = "{\n  // a\n  \"k\": 1, /* b */ \"m\": 2\n}";
        let out = strip_jsonc_comments(input);
        assert!(!out.contains("// a"));
        assert!(!out.contains("/* b */"));
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["k"], 1);
        assert_eq!(v["m"], 2);
    }

    #[test]
    fn preserves_slashes_and_commas_inside_strings() {
        let input = r#"{ "url": "https://x/y", "list": "a,]" }"#;
        let out = sanitize_jsonc(input);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["url"], "https://x/y");
        assert_eq!(v["list"], "a,]");
    }

    #[test]
    fn drops_utf8_bom() {
        let input = "\u{FEFF}{\"k\":1}";
        let out = strip_jsonc_comments(input);
        assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
    }

    #[test]
    fn strips_trailing_comma_before_object_close() {
        let input = "{ \"a\": 1, \"b\": 2, }";
        let out = strip_trailing_commas(input);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn strips_trailing_comma_before_array_close_and_nested() {
        let input = "{ \"tokens\": { \"x\": \"#fff\", }, \"list\": [1, 2, ], }";
        let out = strip_trailing_commas(input);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["tokens"]["x"], "#fff");
        assert_eq!(v["list"][1], 2);
    }

    #[test]
    fn strips_trailing_comma_with_newline_before_brace() {
        let input = "{\n  \"a\": 1,\n  \"b\": 2,\n}";
        let v: serde_json::Value = serde_json::from_str(&sanitize_jsonc(input)).unwrap();
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn leaves_legal_interior_commas_untouched() {
        let input = "{ \"a\": 1, \"b\": 2 }";
        assert_eq!(strip_trailing_commas(input), input);
    }
}
