use serde_json::Value;

/// Mask sensitive patterns in arbitrary text before logging.
///
/// Four pattern families are recognised:
/// 1. Token-shaped values that follow a keyword (`token=…`, `Authorization: …`,
///    `password: …`, etc.) — the trailing value is replaced with `[REDACTED]`.
/// 2. RTMP URLs of the form `rtmp[s]://host[:port]/app/STREAM_KEY` — the
///    trailing path segment (the stream key) is masked. Twitch and YouTube
///    embed the secret key as the last URL segment, so any RTMP URL in
///    logs that survives without masking leaks the broadcast key.
/// 3. FFmpeg-style `${TOKEN}` template expansions — the contents inside
///    the braces are replaced with `[REDACTED]` so command-line dumps
///    of templated args don't ship the secret.
/// 4. `ENC::` (V1) and `ENC2::` (V2) ciphertext blobs — replaced with
///    `[ENCRYPTED]` so encrypted-at-rest values don't appear verbatim.
pub(crate) fn mask_sensitive(text: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    static ENC_RE: OnceLock<Regex> = OnceLock::new();
    static RTMP_RE: OnceLock<Regex> = OnceLock::new();
    static TEMPLATE_RE: OnceLock<Regex> = OnceLock::new();

    let token_re = TOKEN_RE.get_or_init(|| {
        Regex::new(r#"(?i)(token|key|password|secret|bearer|oauth|access_token|refresh_token|authorization)[=:\s]+['"]?([A-Za-z0-9_\-./+]{20,})['"]?"#).unwrap()
    });
    let enc_re = ENC_RE.get_or_init(|| Regex::new(r#"ENC2?::[A-Za-z0-9+/=]{10,}"#).unwrap());
    let rtmp_re = RTMP_RE.get_or_init(|| {
        // rtmp[s]://host[:port]/app/STREAM_KEY[?query]
        // Captures up to and including the application path, then masks
        // the trailing stream-key segment. Allows query strings to
        // survive (useful for debugging) but redacts the key.
        Regex::new(r"(?i)(rtmps?://[^\s/]+(?:/[^\s/?#]+){1,2}/)([^\s/?#]+)").unwrap()
    });
    let template_re = TEMPLATE_RE.get_or_init(|| Regex::new(r"\$\{([^}]+)\}").unwrap());

    let result = token_re.replace_all(text, "$1=[REDACTED]");
    let result = enc_re.replace_all(&result, "[ENCRYPTED]");
    let result = rtmp_re.replace_all(&result, "$1[REDACTED]");
    template_re
        .replace_all(&result, "${[REDACTED]}")
        .to_string()
}

/// Redact sensitive keys from a JSON payload before logging.
pub(crate) fn redact_payload(value: &Value) -> Value {
    const REDACT_KEYS: &[&str] = &[
        "token",
        "key",
        "password",
        "secret",
        "oauth",
        "accessToken",
        "refreshToken",
        "oauthToken",
        "apiKey",
        "access_token",
        "refresh_token",
        "session_token",
        "webhookUrl",
    ];

    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (k, v) in map {
                let lower = k.to_lowercase();
                if REDACT_KEYS
                    .iter()
                    .any(|s| lower.contains(&s.to_lowercase()))
                {
                    if let Value::String(s) = v {
                        if !s.is_empty() {
                            redacted.insert(k.clone(), Value::String("[REDACTED]".to_string()));
                        } else {
                            redacted.insert(k.clone(), v.clone());
                        }
                    } else {
                        redacted.insert(k.clone(), Value::String("[REDACTED]".to_string()));
                    }
                } else {
                    redacted.insert(k.clone(), redact_payload(v));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_payload).collect()),
        other => other.clone(),
    }
}
