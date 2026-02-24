use serde_json::Value;

/// Mask sensitive patterns in text (tokens, keys, passwords, ENC:: values)
pub(crate) fn mask_sensitive(text: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    static ENC_RE: OnceLock<Regex> = OnceLock::new();

    // Match long alphanumeric strings that follow keywords like token, key, password, secret, Bearer
    let token_re = TOKEN_RE.get_or_init(|| {
        Regex::new(r#"(?i)(token|key|password|secret|bearer|oauth|access_token|refresh_token|authorization)[=:\s]+['"]?([A-Za-z0-9_\-./+]{20,})['"]?"#).expect("invalid token redaction regex")
    });
    // Match ENC:: prefixed values
    let enc_re = ENC_RE.get_or_init(|| {
        Regex::new(r#"ENC::[A-Za-z0-9+/=]{10,}"#).expect("invalid ENC redaction regex")
    });

    let result = token_re.replace_all(text, "$1=[REDACTED]");
    enc_re.replace_all(&result, "[ENCRYPTED]").to_string()
}

/// Redact sensitive keys from a JSON payload before logging
pub(crate) fn redact_payload(value: &Value) -> Value {
    const REDACT_KEYS: &[&str] = &[
        "token", "key", "password", "secret", "oauth", "accessToken",
        "refreshToken", "oauthToken", "apiKey", "access_token", "refresh_token",
        "session_token", "webhookUrl",
    ];

    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (k, v) in map {
                let lower = k.to_lowercase();
                if REDACT_KEYS.iter().any(|s| lower.contains(&s.to_lowercase())) {
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

/// Sanitize error messages to prevent information disclosure
pub(crate) fn sanitize_error(error: &str) -> String {
    let masked = mask_sensitive(error);
    log::warn!("[sanitize_error] Original error: {}", masked);
    eprintln!("[sanitize_error] Original error: {}", masked);
    let lower = masked.to_lowercase();

    if lower.contains("failed to read") || lower.contains("no such file") || lower.contains("not found") {
        return "Resource not found".to_string();
    }
    // Chat platform errors - pass through user-friendly messages (using masked version)
    if lower.contains("does not exist on twitch") || lower.contains("channel") && lower.contains("not found") {
        return masked.to_string();
    }
    if lower.contains("failed to connect to") {
        return masked.to_string();
    }
    if lower.contains("no active live broadcast") || lower.contains("not currently live") {
        return masked.to_string();
    }
    if lower.contains("already connected") || lower.contains("not connected") {
        return masked.to_string();
    }
    if lower.contains("no youtube oauth token") || lower.contains("no twitch oauth token") || lower.contains("please sign in") {
        return masked.to_string();
    }
    if lower.contains("parse") || lower.contains("invalid") {
        return "Invalid request format".to_string();
    }
    if lower.contains("permission") || lower.contains("access") || lower.contains("denied") {
        return "Access denied".to_string();
    }
    if lower.contains("traversal") || lower.contains("outside") {
        return "Invalid path".to_string();
    }
    if lower.contains("encrypt") || lower.contains("decrypt") {
        return "Encryption error".to_string();
    }
    // Discord webhook errors - pass through user-friendly messages (using masked version)
    if lower.contains("webhook") || lower.contains("discord") || lower.contains("rate limit") {
        return masked.to_string();
    }
    // Network errors - safe to show (using masked version)
    if lower.contains("request failed") || lower.contains("connection") || lower.contains("timeout") {
        return masked.to_string();
    }
    // Missing argument errors - safe to show (using masked version)
    if lower.contains("missing argument") {
        return masked.to_string();
    }
    // Unknown command errors - safe to show for debugging (using masked version)
    if lower.contains("unknown command") {
        return masked.to_string();
    }

    // Return generic message for unknown errors in production
    // In debug mode, we could log the actual error server-side
    log::debug!("Sanitized error: {masked}");
    "Operation failed".to_string()
}
