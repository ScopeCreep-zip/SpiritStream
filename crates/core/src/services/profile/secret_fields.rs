//! Single source of truth for every machine-key-encrypted field on a
//! [`Profile`].
//!
//! Both the save/load encryption boundary (`profile/security.rs`) and
//! machine-key rotation (`services/encryption/rotation.rs`) must touch
//! exactly the same field set: a field encrypted on save but skipped by
//! rotation is destroyed the moment the old key is shredded (the pre-fix
//! rotation missed `pii_blocklist` and the kick/facebook OAuth tokens
//! for exactly this reason). Routing every consumer through one walker
//! makes that drift structurally impossible — adding a field here adds
//! it to save, load, and rotation in the same change.

use crate::errors::CoreError;
use crate::models::Profile;

/// What kind of secret a visited field holds. Lets callers apply
/// per-kind policy (stream keys respect the per-profile
/// `encrypt_stream_keys` flag; everything else is always encrypted).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SecretFieldKind {
    /// `output_groups[*].stream_targets[*].stream_key`
    StreamKey,
    /// OBS password, Discord webhook URL, backend token, YouTube API key.
    SensitiveSetting,
    /// OAuth access/refresh tokens for every provider.
    OauthToken,
    /// PII blocklist entries (real names, deadnames, hometowns).
    PiiEntry,
}

/// Visit every machine-key-encrypted field of `profile` in place.
///
/// The closure decides what to do with each field (encrypt, decrypt,
/// count, …). Stops at the first error.
pub(crate) fn visit_secret_fields<F>(profile: &mut Profile, mut f: F) -> Result<(), CoreError>
where
    F: FnMut(SecretFieldKind, &mut String) -> Result<(), CoreError>,
{
    for group in &mut profile.output_groups {
        for target in &mut group.stream_targets {
            f(SecretFieldKind::StreamKey, &mut target.stream_key)?;
        }
    }

    f(
        SecretFieldKind::SensitiveSetting,
        &mut profile.settings.obs.password,
    )?;
    f(
        SecretFieldKind::SensitiveSetting,
        &mut profile.settings.discord.webhook_url,
    )?;
    f(
        SecretFieldKind::SensitiveSetting,
        &mut profile.settings.backend.token,
    )?;
    f(
        SecretFieldKind::SensitiveSetting,
        &mut profile.settings.chat.youtube_api_key,
    )?;

    let oauth = &mut profile.settings.oauth;
    for account in [
        &mut oauth.twitch,
        &mut oauth.youtube,
        &mut oauth.kick,
        &mut oauth.facebook,
    ] {
        f(SecretFieldKind::OauthToken, &mut account.access_token)?;
        f(SecretFieldKind::OauthToken, &mut account.refresh_token)?;
    }

    for entry in &mut profile.pii_blocklist {
        f(SecretFieldKind::PiiEntry, entry)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{OutputGroup, ProfileSettings, StreamTarget};

    fn target(id: &str, stream_key: &str) -> StreamTarget {
        StreamTarget {
            id: id.into(),
            service: Default::default(),
            name: id.into(),
            url: String::new(),
            stream_key: stream_key.into(),
            enabled: true,
        }
    }

    fn profile_with_every_secret_populated() -> Profile {
        let mut settings = ProfileSettings::default();
        settings.obs.password = "obs-pw".into();
        settings.discord.webhook_url = "https://discord/wh".into();
        settings.backend.token = "backend-token".into();
        settings.chat.youtube_api_key = "yt-key".into();
        for (i, account) in [
            &mut settings.oauth.twitch,
            &mut settings.oauth.youtube,
            &mut settings.oauth.kick,
            &mut settings.oauth.facebook,
        ]
        .into_iter()
        .enumerate()
        {
            account.access_token = format!("access-{i}");
            account.refresh_token = format!("refresh-{i}");
        }
        Profile {
            id: "walker-test".into(),
            name: "walker-test".into(),
            encrypted: false,
            input: crate::models::RtmpInput::default(),
            output_groups: vec![OutputGroup {
                stream_targets: vec![target("t1", "sk-1"), target("t2", "sk-2")],
                ..OutputGroup::default()
            }],
            settings,
            pii_blocklist: vec!["Alice Smith".into(), "Springfield".into()],
            pii_fuzzy: false,
            anonymous_logging: false,
            anonymous_salt: String::new(),
        }
    }

    /// Drift guard: the walker must visit every secret field exactly
    /// once. If a new secret field is added to `Profile` without being
    /// added to the walker, the count here is the place that catches a
    /// reviewer's eye — update the walker, not just this constant.
    #[test]
    fn walker_visits_every_secret_field_exactly_once() {
        let mut profile = profile_with_every_secret_populated();
        let mut visited: Vec<(SecretFieldKind, String)> = Vec::new();
        visit_secret_fields(&mut profile, |kind, field| {
            visited.push((kind, field.clone()));
            Ok(())
        })
        .unwrap();

        let count = |kind: SecretFieldKind| visited.iter().filter(|(k, _)| *k == kind).count();
        assert_eq!(count(SecretFieldKind::StreamKey), 2);
        assert_eq!(count(SecretFieldKind::SensitiveSetting), 4);
        assert_eq!(count(SecretFieldKind::OauthToken), 8);
        assert_eq!(count(SecretFieldKind::PiiEntry), 2);
        assert_eq!(visited.len(), 16);

        // Every populated value reached the closure verbatim.
        for expected in [
            "sk-1",
            "obs-pw",
            "access-2",  // kick
            "refresh-3", // facebook
            "Alice Smith",
        ] {
            assert!(
                visited.iter().any(|(_, v)| v == expected),
                "walker missed field value {expected:?}",
            );
        }
    }

    #[test]
    fn walker_mutations_land_on_the_profile() {
        let mut profile = profile_with_every_secret_populated();
        visit_secret_fields(&mut profile, |_, field| {
            *field = format!("X{field}");
            Ok(())
        })
        .unwrap();
        assert_eq!(profile.output_groups[0].stream_targets[0].stream_key, "Xsk-1");
        assert_eq!(profile.settings.oauth.facebook.refresh_token, "Xrefresh-3");
        assert_eq!(profile.pii_blocklist[1], "XSpringfield");
    }
}
