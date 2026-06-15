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

/// One numbered instruction in the guided setup. When the action happens on
/// a specific console page the user would otherwise have to hunt for (e.g.
/// Google's separate "Audience" / "Data Access" pages), `url` carries the
/// direct deep-link so the frontend can show a copy-the-link button — the
/// webview can't open external links itself.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct OAuthStep {
    /// Plain-language instruction, written for a non-technical user.
    pub text: String,
    /// Direct link to the exact page this step happens on, if any.
    pub url: Option<String>,
    /// Values to paste AT THIS step, shown inline right where the
    /// instruction asks for them (e.g. the App name on the "name the app"
    /// step) — not collected at the bottom away from their context.
    pub fields: Vec<OAuthConsoleField>,
}

/// Everything the in-app guided setup needs to walk a user through
/// registering an OAuth app for one provider without thinking about it.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct OAuthProviderSetup {
    /// Numbered instructions shown before the paste-values. Empty for
    /// providers whose console is a single create-app form.
    pub steps: Vec<OAuthStep>,
    /// The labeled values to paste/select into the console form, in order.
    pub console_fields: Vec<OAuthConsoleField>,
}

fn step(text: &str, url: Option<&str>) -> OAuthStep {
    OAuthStep {
        text: text.to_string(),
        url: url.map(str::to_string),
        fields: vec![],
    }
}

/// A step that asks the user to paste one or more values, shown inline.
fn step_with(text: &str, url: Option<&str>, fields: Vec<OAuthConsoleField>) -> OAuthStep {
    OAuthStep {
        text: text.to_string(),
        url: url.map(str::to_string),
        fields,
    }
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
    let name = field("Name", app_name.clone(), true, Some(name_note));

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
        // Google's 2025+ console reorganised OAuth setup into the "Google
        // Auth Platform" (a first-run Get-started wizard, then Branding /
        // Audience / Data Access / Clients pages). A non-technical user can't
        // find these from a menu, so every step carries its OWN direct
        // deep-link AND the link renders before the instruction (go there
        // first, then do the thing). Two gotchas drive the wording: the scope
        // lives in a "Manually add scopes" box at the BOTTOM of Data Access
        // (paste → "Add to table" → Update → Save), and Testing-mode tokens
        // die after 7 days unless the app is published.
        "youtube" => OAuthProviderSetup {
            steps: vec![
                step(
                    "Turn the API on — click the blue \"Enable\" button on the page that opens.",
                    Some("https://console.cloud.google.com/apis/library/youtube.googleapis.com"),
                ),
                step_with(
                    "Start the setup wizard — click \"Get started\", then enter this App name and your email, and when it asks for an audience pick \"External\". Finish the short wizard.",
                    Some("https://console.cloud.google.com/auth/overview"),
                    vec![field("App name", app_name, true, None)],
                ),
                step_with(
                    "Add the permission — click \"Add or remove scopes\", scroll to the \"Manually add scopes\" box at the bottom, paste this in, click \"Add to table\", then \"Update\" and \"Save\".",
                    Some("https://console.cloud.google.com/auth/scopes"),
                    vec![field(
                        "Scope",
                        "https://www.googleapis.com/auth/youtube.force-ssl",
                        true,
                        None,
                    )],
                ),
                step(
                    "Let yourself in — under \"Test users\" click \"Add users\", add your Google email, and Save. In Testing mode sign-in stops working after 7 days, so click \"Publish app\" on this page to make it permanent.",
                    Some("https://console.cloud.google.com/auth/audience"),
                ),
                step(
                    "Create the login — click \"Create client\", set Application type to \"Desktop app\", click \"Create\", then copy the Client ID it shows you.",
                    Some("https://console.cloud.google.com/auth/clients"),
                ),
                step(
                    "One thing to expect when you sign in: because it's your own brand-new app, Google warns \"Google hasn't verified this app\". Click \"Advanced\", then \"Go to … (unsafe)\" — it's safe, it's yours.",
                    None,
                ),
            ],
            console_fields: vec![],
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
            steps: vec![step(
                "Apply for an app on the Trovo Open Platform — approval is manual, so this one isn't instant.",
                None,
            )],
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
            steps: vec![step(
                "Create a Business app, then add the \"Facebook Login\" product.",
                None,
            )],
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
    fn youtube_guidance_has_direct_links_and_the_token_caveat() {
        let s = console_setup("youtube", Some("ch"));
        // No redirect (desktop loopback).
        assert!(
            !s.console_fields.iter().any(|f| f.label.contains("Redirect")),
            "desktop-app clients have no redirect to paste"
        );
        // Real guidance, not a stub: the scope to paste is shown INLINE at
        // its step (Add the permission), not collected at the bottom.
        assert!(
            s.steps
                .iter()
                .flat_map(|st| &st.fields)
                .any(|f| f.value.contains("youtube.force-ssl")),
            "the youtube.force-ssl scope must be attached to its step"
        );
        assert!(
            s.console_fields.is_empty(),
            "youtube values live on their steps, not the trailing list"
        );
        // Every step that means "go somewhere" carries a direct deep-link —
        // non-technical users can't find Google's separate pages from a menu.
        let with_links = s.steps.iter().filter(|st| st.url.is_some()).count();
        assert!(
            with_links >= 4,
            "each console page (enable API, branding, audience, scopes, clients) needs its own link"
        );
        assert!(s
            .steps
            .iter()
            .any(|st| st.url.as_deref() == Some("https://console.cloud.google.com/auth/audience")));
        // Current console flow (2025+): first-run setup is the "Get started"
        // wizard on the Auth-Platform overview, not the old standalone
        // Branding page.
        assert!(
            s.steps
                .iter()
                .any(|st| st.url.as_deref()
                    == Some("https://console.cloud.google.com/auth/overview")),
            "first-run setup goes through the Get-started wizard on /auth/overview"
        );
        // The 7-day testing-token expiry / publish caveat — the single most
        // important thing for staying signed in.
        assert!(
            s.steps.iter().any(|st| st.text.contains("7 days"))
                && s.steps.iter().any(|st| st.text.contains("Publish app")),
            "must warn about the Testing-mode token expiry and the publish fix"
        );
        // The exact-label nuances a non-technical user can't infer: the scope
        // is added via "Add to table", the client is a "Desktop app", and the
        // browser will flag the unverified app (click "Advanced").
        let all_text: String = s.steps.iter().map(|st| st.text.as_str()).collect();
        assert!(
            all_text.contains("Add to table"),
            "scope step must name the \"Add to table\" button"
        );
        assert!(
            all_text.contains("Desktop app"),
            "client step must specify the \"Desktop app\" application type"
        );
        assert!(
            all_text.contains("Advanced"),
            "must prep the user for Google's unverified-app warning (\"Advanced\")"
        );
    }
}
