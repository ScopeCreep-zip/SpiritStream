//! Guided developer-console setup data.
//!
//! Registering an OAuth app is the one step that genuinely can't live
//! inside SpiritStream — it happens on the provider's own site. This
//! module makes that step thoughtless: the backend pre-computes every
//! value the user pastes (the app name derived from their channel, the
//! redirect URL, the category, the client type), so the in-app form is
//! just labeled rows with copy buttons. All provider knowledge stays
//! here; the frontend renders strings it's handed.
//!
//! Console field values verified June 2026 against each provider's
//! portal (notably: Twitch's form rejects a bare `http://localhost` —
//! the localhost-over-HTTP exception only applies with a port).

use serde::Serialize;
use ts_rs::TS;

/// One value the user pastes (or option they select) in a provider's
/// developer console while registering an app.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct OAuthConsoleField {
    /// The console's own field label, e.g. "Name" or "OAuth Redirect URL".
    pub label: String,
    /// The exact value to paste, or the option to choose.
    pub value: String,
    /// `true` → render a copy button (a value to paste); `false` → it's
    /// a dropdown choice the user selects, nothing to copy.
    pub copyable: bool,
    /// One-line caveat shown under the field, e.g. "Must be unique".
    pub note: Option<String>,
}

/// Everything the in-app guided setup needs to walk a user through
/// registering an OAuth app for one provider without thinking about it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct OAuthProviderSetup {
    /// Plain-language steps shown before the fields (e.g. "Enable the
    /// YouTube Data API v3"). Empty for providers whose console is a
    /// single create-app form.
    pub steps: Vec<String>,
    /// The labeled values to paste/select into the console form, in order.
    pub console_fields: Vec<OAuthConsoleField>,
}

fn field(label: &str, value: impl Into<String>, copyable: bool, note: Option<&str>) -> OAuthConsoleField {
    OAuthConsoleField {
        label: label.to_string(),
        value: value.into(),
        copyable,
        note: note.map(str::to_string),
    }
}

/// Build the guided console-setup for `provider`, pre-filling the app
/// name from the user's channel when known. `channel_hint` is the
/// channel/username the user already entered for this platform (read
/// from the active profile by the transport).
pub fn console_setup(provider: &str, channel_hint: Option<&str>) -> OAuthProviderSetup {
    let channel = channel_hint.map(str::trim).filter(|s| !s.is_empty());
    let app_name = match channel {
        Some(ch) => format!("SpiritStream - {ch}"),
        None => "SpiritStream".to_string(),
    };
    let name_note = if channel.is_some() {
        "Your channel name keeps this unique across the platform."
    } else {
        "Names must be globally unique — set your channel above and this personalises itself."
    };
    let name = field("Name", app_name, true, Some(name_note));

    match provider {
        // Public client / Device Code Flow — no real redirect, but the
        // form demands one. The localhost-over-HTTP exception needs a
        // port; bare `http://localhost` is rejected.
        "twitch" => OAuthProviderSetup {
            steps: vec![],
            console_fields: vec![
                name,
                field(
                    "OAuth Redirect URL",
                    "http://localhost:3000",
                    true,
                    Some("Required by the form but never used. Keep the port. Fill this one field and Save — don't click \"Add\"."),
                ),
                field("Category", "Application Integration", false, None),
                field(
                    "Client Type",
                    "Public",
                    false,
                    Some("Important — Public means no secret, which is what SpiritStream uses."),
                ),
            ],
        },
        // Google Desktop-app client: loopback is automatic, so there's
        // no redirect to paste. The work is enabling the API + consent.
        "youtube" => OAuthProviderSetup {
            steps: vec![
                "Create or pick a project, then configure the OAuth consent screen.".to_string(),
                "Enable \"YouTube Data API v3\" under APIs & Services → Library.".to_string(),
                "Open Credentials → Create credentials → OAuth client ID.".to_string(),
            ],
            console_fields: vec![
                field(
                    "Application type",
                    "Desktop app",
                    false,
                    Some("No redirect URL needed — desktop clients use loopback automatically."),
                ),
                name,
            ],
        },
        "kick" => OAuthProviderSetup {
            steps: vec![],
            console_fields: vec![
                name,
                field(
                    "Redirect URI",
                    "http://localhost:8891/oauth/callback",
                    true,
                    Some("Paste exactly. If the form accepts several, add ports 8891 through 8895."),
                ),
            ],
        },
        "trovo" => OAuthProviderSetup {
            steps: vec![
                "Apply for an app on the Trovo Open Platform — approval is manual.".to_string(),
            ],
            console_fields: vec![
                name,
                field(
                    "Redirect URI",
                    "http://localhost:8891/oauth/callback",
                    true,
                    Some("Must match exactly."),
                ),
            ],
        },
        "facebook" => OAuthProviderSetup {
            steps: vec![
                "Create a Business app, then add the \"Facebook Login\" product.".to_string(),
            ],
            console_fields: vec![
                name,
                field(
                    "Valid OAuth Redirect URI",
                    "http://localhost:8891/oauth/callback",
                    true,
                    Some("Under Facebook Login → Settings."),
                ),
            ],
        },
        _ => OAuthProviderSetup {
            steps: vec![],
            console_fields: vec![name],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_name_uses_channel_when_present() {
        let s = console_setup("twitch", Some("spiritartlife"));
        let name = &s.console_fields[0];
        assert_eq!(name.label, "Name");
        assert_eq!(name.value, "SpiritStream - spiritartlife");
        assert!(name.copyable);
    }

    #[test]
    fn app_name_falls_back_without_channel() {
        let s = console_setup("twitch", None);
        assert_eq!(s.console_fields[0].value, "SpiritStream");
        // Blank/whitespace hints are treated as absent.
        let s2 = console_setup("twitch", Some("   "));
        assert_eq!(s2.console_fields[0].value, "SpiritStream");
    }

    #[test]
    fn twitch_redirect_keeps_the_port_and_is_public() {
        let s = console_setup("twitch", Some("ch"));
        let redirect = s
            .console_fields
            .iter()
            .find(|f| f.label == "OAuth Redirect URL")
            .expect("twitch has a redirect field");
        assert_eq!(redirect.value, "http://localhost:3000");
        let client_type = s
            .console_fields
            .iter()
            .find(|f| f.label == "Client Type")
            .expect("twitch has a client-type field");
        assert_eq!(client_type.value, "Public");
        assert!(!client_type.copyable, "client type is a selection, not a paste");
    }

    #[test]
    fn loopback_providers_paste_the_8891_callback() {
        for provider in ["kick", "trovo", "facebook"] {
            let s = console_setup(provider, Some("ch"));
            assert!(
                s.console_fields
                    .iter()
                    .any(|f| f.value == "http://localhost:8891/oauth/callback"),
                "{provider} must surface the loopback callback URL"
            );
        }
    }

    #[test]
    fn youtube_has_no_redirect_and_lists_api_step() {
        let s = console_setup("youtube", Some("ch"));
        assert!(s.steps.iter().any(|step| step.contains("YouTube Data API v3")));
        assert!(
            !s.console_fields.iter().any(|f| f.label.contains("Redirect")),
            "desktop-app clients have no redirect to paste"
        );
    }
}
