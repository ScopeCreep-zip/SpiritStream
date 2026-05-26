//! Theme parsing, token-set requirements, and the JSONC depth cap.
//!
//! Parse + validate are split from the catalog/install entry points
//! so they can be unit-tested in isolation and audited as a unit. The
//! brace-depth scanner and JSONC comment stripper are also exposed
//! here as free functions; both are stateless and CPU-only.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use regex::Regex;

use crate::errors::CoreError;
use crate::models::ThemeFile;

use super::{
    theme_invalid, MAX_THEME_JSONC_DEPTH, REQUIRED_TOKENS, THEME_ID_PATTERN, THEME_ID_REGEX,
    TOKENS_CSS,
};

impl super::ThemeManager {}

pub(super) fn load_theme_file(path: &Path) -> Result<ThemeFile, CoreError> {
    let content = fs::read_to_string(path).map_err(|e| CoreError::Internal {
        context: format!("Failed to read theme: {e}"),
    })?;
    let mut theme = parse_theme(&content)?;
    apply_token_fallbacks(&mut theme.tokens);
    validate_theme(&theme)?;
    Ok(theme)
}

pub(super) fn parse_theme(content: &str) -> Result<ThemeFile, CoreError> {
    let sanitized = strip_jsonc_comments(content);
    // Reject inputs whose nesting depth exceeds the policy cap before
    // handing them to `serde_json`. Themes are flat token maps (depth
    // 1-3 in practice). `serde_json` itself bounds recursion at 128, so
    // this is defense-in-depth that catches stack-blow attempts well
    // before the parser does.
    if let Some(depth) = max_brace_depth_or_overflow(&sanitized, MAX_THEME_JSONC_DEPTH) {
        return Err(theme_invalid(format!(
            "Theme JSONC nesting depth {depth} exceeds limit {MAX_THEME_JSONC_DEPTH}"
        )));
    }
    serde_json::from_str(&sanitized)
        .map_err(|e| theme_invalid(format!("Invalid theme JSON: {e}")))
}

pub(super) fn apply_token_fallbacks(tokens: &mut HashMap<String, String>) {
    if !tokens.contains_key("--border-subtle") {
        if let Some(border_muted) = tokens.get("--border-muted").cloned() {
            tokens.insert("--border-subtle".to_string(), border_muted);
        }
    }
}

pub(super) fn validate_theme(theme: &ThemeFile) -> Result<(), CoreError> {
    if theme.id.trim().is_empty() {
        return Err(theme_invalid("Theme id is required"));
    }
    if theme.name.trim().is_empty() {
        return Err(theme_invalid("Theme name is required"));
    }

    let id_regex = THEME_ID_REGEX.get_or_init(|| Regex::new(THEME_ID_PATTERN).unwrap());
    if !id_regex.is_match(&theme.id) {
        return Err(theme_invalid(
            "Theme id must be lowercase alphanumeric with dashes or underscores",
        ));
    }

    if theme.tokens.is_empty() {
        return Err(theme_invalid("Theme tokens cannot be empty"));
    }

    let required = required_tokens();
    let mode_label = theme.mode.as_str();
    validate_token_set(mode_label, &theme.tokens, required)?;

    Ok(())
}

fn validate_token_set(
    label: &str,
    tokens: &HashMap<String, String>,
    required: &[String],
) -> Result<(), CoreError> {
    let missing: Vec<&String> = required
        .iter()
        .filter(|key| !tokens.contains_key(*key))
        .collect();
    if !missing.is_empty() {
        let preview = missing
            .iter()
            .take(5)
            .map(|k| (*k).as_str())
            .collect::<Vec<_>>();
        let remaining = missing.len().saturating_sub(preview.len());
        let suffix = if remaining > 0 {
            format!(" (and {remaining} more)")
        } else {
            "".to_string()
        };

        return Err(theme_invalid(format!(
            "Missing {label} tokens: {}{suffix}",
            preview.join(", ")
        )));
    }

    for (key, value) in tokens {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(theme_invalid(format!(
                "Invalid {label} token '{key}': value cannot be empty"
            )));
        }
        if trimmed.contains("REPLACE_ME") {
            return Err(theme_invalid(format!(
                "Invalid {label} token '{key}': replace REPLACE_ME placeholders"
            )));
        }
        if trimmed.contains("</style>") || trimmed.contains("<script") {
            return Err(theme_invalid(format!(
                "Invalid {label} token '{key}': value contains dangerous content"
            )));
        }
    }

    Ok(())
}

pub(super) fn required_tokens() -> &'static Vec<String> {
    REQUIRED_TOKENS.get_or_init(|| {
        let token_regex = Regex::new(r"--[A-Za-z0-9_-]+").unwrap();
        let mut tokens = HashSet::new();

        for cap in token_regex.captures_iter(TOKENS_CSS) {
            if let Some(matched) = cap.get(0) {
                let token = matched.as_str();

                // Skip optional tokens that themes can customize or omit:
                // color scales (violet-*, fuchsia-*, etc.) and typography
                // variants (font-*, letter-spacing-*, line-height-*).
                let is_optional = token.starts_with("--violet-")
                    || token.starts_with("--fuchsia-")
                    || token.starts_with("--pink-")
                    || token.starts_with("--neutral-")
                    || token.starts_with("--purple-")
                    || token.starts_with("--cyan-")
                    || token.starts_with("--green-")
                    || token.starts_with("--orange-")
                    || token.starts_with("--red-")
                    || token.starts_with("--yellow-")
                    || token.starts_with("--font-")
                    || token.starts_with("--letter-spacing-")
                    || token.starts_with("--line-height-");

                if !is_optional {
                    tokens.insert(token.to_string());
                }
            }
        }

        let mut tokens: Vec<String> = tokens.into_iter().collect();
        tokens.sort();
        tokens
    })
}

/// Scan `input` and return the maximum brace/bracket nesting depth, or
/// `None` if depth never exceeds `limit`. String literals are skipped so
/// `{` inside a string doesn't count. Used to reject stack-blow attempts
/// before they reach `serde_json`.
pub(super) fn max_brace_depth_or_overflow(input: &str, limit: u32) -> Option<u32> {
    let mut depth: u32 = 0;
    let mut max_seen: u32 = 0;
    let mut in_string = false;
    let mut escape = false;
    for byte in input.bytes() {
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.saturating_add(1);
                if depth > max_seen {
                    max_seen = depth;
                }
                if depth > limit {
                    return Some(depth);
                }
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    None
}

fn strip_jsonc_comments(input: &str) -> String {
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

#[cfg(test)]
mod phase_6_10_tests {
    use super::*;

    #[test]
    fn flat_theme_passes_depth_check() {
        let theme = r#"{ "id": "x", "tokens": { "a": "b", "c": "d" } }"#;
        assert_eq!(
            max_brace_depth_or_overflow(theme, MAX_THEME_JSONC_DEPTH),
            None
        );
    }

    #[test]
    fn deeply_nested_input_is_rejected() {
        let mut payload = String::new();
        for _ in 0..40 {
            payload.push('{');
        }
        payload.push_str("\"x\":1");
        for _ in 0..40 {
            payload.push('}');
        }
        let result = max_brace_depth_or_overflow(&payload, MAX_THEME_JSONC_DEPTH);
        assert!(
            matches!(result, Some(d) if d > MAX_THEME_JSONC_DEPTH),
            "40-deep input must exceed the {MAX_THEME_JSONC_DEPTH} cap",
        );
    }

    #[test]
    fn braces_inside_strings_dont_count() {
        // `{` inside a JSON string literal must NOT inflate the depth.
        // 40 inside a string, only 1 real outer object.
        let mut payload = String::from("{ \"value\": \"");
        for _ in 0..40 {
            payload.push('{');
        }
        payload.push_str("\" }");
        assert_eq!(
            max_brace_depth_or_overflow(&payload, MAX_THEME_JSONC_DEPTH),
            None
        );
    }

    #[test]
    fn parse_theme_rejects_oversized_nesting() {
        let mut payload = String::new();
        for _ in 0..40 {
            payload.push('{');
        }
        payload.push_str("\"x\":1");
        for _ in 0..40 {
            payload.push('}');
        }
        let err = parse_theme(&payload).unwrap_err();
        let CoreError::ValidationFailed { reasons } = err else {
            panic!("expected ValidationFailed, got {err:?}");
        };
        assert!(
            reasons.iter().any(|r| r.message.contains("nesting depth")),
            "expected depth error in reasons, got {reasons:?}",
        );
    }
}
