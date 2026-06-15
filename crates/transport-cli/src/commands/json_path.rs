//! Dotted-path JSON mutation shared by `settings set` and `profile set`.
//!
//! Both edit a persisted document by serializing it to a `serde_json::Value`,
//! applying `key=value` overrides, then re-typing it back through serde (which
//! validates the shape). The single difference is depth: global settings are a
//! flat top-level object; profile settings nest (`obs.host`, `discord.webhookEnabled`).
//! `set_dotted` handles both — a one-segment key is just the flat case.

use serde_json::Value;

use crate::error::CliError;

/// Set `value` at a dotted key path inside a JSON object, descending through
/// nested objects. Every segment — intermediate AND the leaf — must already
/// exist, so a typo (`obs.hostt`) fails loudly with an EX_USAGE `argument`
/// error instead of silently inventing a key the typed model would then drop.
pub fn set_dotted(root: &mut Value, key: &str, value: Value) -> Result<(), CliError> {
    let segments: Vec<&str> = key.split('.').filter(|s| !s.is_empty()).collect();
    let Some((leaf, parents)) = segments.split_last() else {
        return Err(CliError::Argument("empty settings key".into()));
    };

    let mut cursor = root;
    for seg in parents {
        let obj = cursor
            .as_object_mut()
            .ok_or_else(|| CliError::Argument(format!("unknown settings key: {key}")))?;
        if !obj.contains_key(*seg) {
            return Err(CliError::Argument(format!("unknown settings key: {key}")));
        }
        // Reborrow into the child for the next descent.
        cursor = obj.get_mut(*seg).expect("contains_key checked above");
    }

    let obj = cursor
        .as_object_mut()
        .ok_or_else(|| CliError::Argument(format!("unknown settings key: {key}")))?;
    if !obj.contains_key(*leaf) {
        return Err(CliError::Argument(format!("unknown settings key: {key}")));
    }
    obj.insert((*leaf).to_owned(), value);
    Ok(())
}

/// Parse a single `key=value` override into its trimmed key and a JSON value
/// (numbers/booleans/objects parse as JSON; anything else falls back to a raw
/// string). Shared so `settings set` and `profile set` accept identical syntax.
pub fn parse_override(raw: &str) -> Result<(&str, Value), CliError> {
    let (key, value_str) = raw
        .split_once('=')
        .ok_or_else(|| CliError::Argument(format!("expected key=value, got: {raw}")))?;
    let value_str = value_str.trim();
    let parsed: Value =
        serde_json::from_str(value_str).unwrap_or_else(|_| Value::String(value_str.to_owned()));
    Ok((key.trim(), parsed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sets_nested_leaf() {
        let mut v = json!({ "obs": { "host": "localhost", "port": 4455 } });
        set_dotted(&mut v, "obs.host", json!("192.168.1.42")).unwrap();
        assert_eq!(v["obs"]["host"], json!("192.168.1.42"));
        assert_eq!(v["obs"]["port"], json!(4455));
    }

    #[test]
    fn sets_top_level_leaf() {
        let mut v = json!({ "logRetentionDays": 7 });
        set_dotted(&mut v, "logRetentionDays", json!(30)).unwrap();
        assert_eq!(v["logRetentionDays"], json!(30));
    }

    #[test]
    fn unknown_leaf_is_rejected() {
        let mut v = json!({ "obs": { "host": "x" } });
        assert!(set_dotted(&mut v, "obs.hostt", json!("y")).is_err());
    }

    #[test]
    fn unknown_parent_is_rejected() {
        let mut v = json!({ "obs": { "host": "x" } });
        assert!(set_dotted(&mut v, "discord.webhookEnabled", json!(true)).is_err());
    }

    #[test]
    fn parse_override_json_and_string_fallback() {
        assert_eq!(
            parse_override("obs.port=4455").unwrap(),
            ("obs.port", json!(4455))
        );
        assert_eq!(
            parse_override("obs.autoConnect=true").unwrap(),
            ("obs.autoConnect", json!(true))
        );
        assert_eq!(
            parse_override("obs.host=192.168.1.42").unwrap(),
            ("obs.host", json!("192.168.1.42"))
        );
    }
}
