//! Regression tests for the two data-loss bugs in machine-key rotation:
//!
//! 1. Rotation re-encrypted a strict subset of the fields the save path
//!    encrypts (missing `pii_blocklist` + kick/facebook OAuth tokens) —
//!    shredding the old key then destroyed those fields permanently.
//! 2. Rotation was not crash-safe: the new key lived only in RAM until
//!    after the old key was deleted, so a crash mid-rotation either
//!    bricked the install or silently minted a fresh key.

use std::collections::HashMap;

use tempfile::TempDir;

use crate::models::{OutputGroup, Platform, Profile, ProfileSettings, RtmpInput, StreamTarget};
use crate::services::ProfileManager;

use super::{Encryption, RotationRecovery, KEY_LEN};

fn full_secret_profile(name: &str, port: u16) -> Profile {
    let mut settings = ProfileSettings {
        encrypt_stream_keys: true,
        ..ProfileSettings::default()
    };
    settings.obs.password = "obs-pw".into();
    settings.discord.webhook_url = "https://discord.example/wh".into();
    settings.backend.token = "backend-token".into();
    settings.chat.youtube_api_key = "yt-api-key".into();
    settings.oauth.twitch.access_token = "tw-access".into();
    settings.oauth.twitch.refresh_token = "tw-refresh".into();
    settings.oauth.youtube.access_token = "yt-access".into();
    settings.oauth.youtube.refresh_token = "yt-refresh".into();
    settings.oauth.kick.access_token = "kk-access".into();
    settings.oauth.kick.refresh_token = "kk-refresh".into();
    settings.oauth.facebook.access_token = "fb-access".into();
    settings.oauth.facebook.refresh_token = "fb-refresh".into();

    let mut og = OutputGroup::new();
    og.id = format!("{name}-g1");
    og.stream_targets = vec![StreamTarget {
        id: format!("{name}-t1"),
        service: Platform::Twitch,
        name: "Twitch".into(),
        url: "rtmp://localhost/x".into(),
        stream_key: "live_stream_key_1234".into(),
    }];

    Profile {
        id: name.into(),
        name: name.into(),
        encrypted: false,
        input: RtmpInput {
            input_type: "rtmp".into(),
            bind_address: "127.0.0.1".into(),
            port,
            application: "live".into(),
        },
        output_groups: vec![og],
        settings,
        pii_blocklist: vec!["Alice Realname".into(), "Springfield".into()],
        pii_fuzzy: false,
        anonymous_logging: true,
        anonymous_salt: String::new(),
    }
}

fn assert_all_secrets_intact(p: &Profile) {
    assert_eq!(
        p.output_groups[0].stream_targets[0].stream_key,
        "live_stream_key_1234"
    );
    assert_eq!(p.settings.obs.password, "obs-pw");
    assert_eq!(p.settings.discord.webhook_url, "https://discord.example/wh");
    assert_eq!(p.settings.backend.token, "backend-token");
    assert_eq!(p.settings.chat.youtube_api_key, "yt-api-key");
    assert_eq!(p.settings.oauth.twitch.access_token, "tw-access");
    assert_eq!(p.settings.oauth.youtube.refresh_token, "yt-refresh");
    assert_eq!(p.settings.oauth.kick.access_token, "kk-access");
    assert_eq!(p.settings.oauth.kick.refresh_token, "kk-refresh");
    assert_eq!(p.settings.oauth.facebook.access_token, "fb-access");
    assert_eq!(p.settings.oauth.facebook.refresh_token, "fb-refresh");
    assert_eq!(
        p.pii_blocklist,
        vec!["Alice Realname".to_string(), "Springfield".to_string()]
    );
}

/// THE data-loss regression: every machine-key-encrypted field — PII
/// blocklist entries and kick/facebook tokens included — must survive
/// rotation. Pre-fix, rotation skipped them and the old-key shred made
/// them unrecoverable (profile load then failed outright).
#[tokio::test]
async fn rotate_preserves_every_secret_field_including_pii_and_all_providers() {
    let data_dir = TempDir::new().unwrap();
    let profiles_dir = data_dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    let mgr = ProfileManager::new(data_dir.path().to_path_buf());

    mgr.save_with_key_encryption(&full_secret_profile("plain", 1935), None)
        .await
        .unwrap();
    mgr.save_with_key_encryption(
        &full_secret_profile("enc", 1936),
        Some("correct-horse-battery"),
    )
    .await
    .unwrap();

    let mut passwords = HashMap::new();
    passwords.insert("enc".to_string(), "correct-horse-battery".to_string());
    let report = Encryption::rotate_machine_key(data_dir.path(), &profiles_dir, &passwords)
        .expect("rotation should succeed");
    assert_eq!(report.profiles_updated, 2);
    // 1 stream key + 4 sensitive settings + 8 oauth tokens + 2 pii entries
    // per profile = 15 each.
    assert_eq!(report.keys_reencrypted, 30, "rotation must cover ALL secret fields");

    assert_all_secrets_intact(&mgr.load_with_key_decryption("plain", None).await.unwrap());
    assert_all_secrets_intact(
        &mgr.load_with_key_decryption("enc", Some("correct-horse-battery"))
            .await
            .unwrap(),
    );

    // No journal left behind.
    assert!(!data_dir.path().join(".stream_key.new").exists());
    assert_eq!(
        std::fs::metadata(data_dir.path().join(".stream_key"))
            .unwrap()
            .len(),
        KEY_LEN as u64
    );
}

/// Crash window A: pending key written, profiles possibly mid-rewrite,
/// old key intact. Recovery must roll back to the backup + old key.
#[tokio::test]
async fn recover_rolls_back_when_old_key_is_intact() {
    let data_dir = TempDir::new().unwrap();
    let profiles_dir = data_dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    let mgr = ProfileManager::new(data_dir.path().to_path_buf());
    mgr.save_with_key_encryption(&full_secret_profile("p", 1935), None)
        .await
        .unwrap();

    // Simulate the crash state: backup taken, pending key journaled,
    // profile file scribbled mid-rewrite.
    super::rotation_backup::backup_profiles_directory(data_dir.path()).unwrap();
    let pending = data_dir.path().join(".stream_key.new");
    crate::services::write_owner_only_atomic(&pending, &[7u8; KEY_LEN]).unwrap();
    std::fs::write(profiles_dir.join("p.json"), b"{ torn mid-rewrite").unwrap();

    // While the journal exists, key access must refuse rather than mint.
    let err = Encryption::get_or_create_machine_key_public(data_dir.path()).unwrap_err();
    assert!(format!("{err:?}").contains("rotation"), "got: {err:?}");

    let outcome = Encryption::recover_interrupted_rotation(data_dir.path()).unwrap();
    assert_eq!(outcome, RotationRecovery::RolledBack);
    assert!(!pending.exists());

    // Profile restored from backup, decryptable under the OLD key.
    assert_all_secrets_intact(&mgr.load_with_key_decryption("p", None).await.unwrap());
}

/// Crash window B: old key already shredded (zero-length marker), pending
/// key present. Profiles were fully rewritten, so recovery must promote
/// the pending key and the rotation completes.
#[tokio::test]
async fn recover_promotes_pending_key_when_old_key_was_shredded() {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use zeroize::Zeroizing;

    let data_dir = TempDir::new().unwrap();
    let profiles_dir = data_dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    let mgr = ProfileManager::new(data_dir.path().to_path_buf());
    mgr.save_with_key_encryption(&full_secret_profile("p", 1935), None)
        .await
        .unwrap();

    // Simulate the crash state: rewrite the profile's secrets under the
    // pending key, journal it, shred the old key to the 0-byte marker.
    let new_key: Zeroizing<[u8; KEY_LEN]> = Zeroizing::new([9u8; KEY_LEN]);
    let json = std::fs::read_to_string(profiles_dir.join("p.json")).unwrap();
    let mut profile: Profile = serde_json::from_str(&json).unwrap();
    crate::services::profile::secret_fields::visit_secret_fields(&mut profile, |_, field| {
        if let Some(encoded) = field.strip_prefix(super::STREAM_KEY_PREFIX_V2) {
            let old_key = Encryption::get_or_create_machine_key_public(data_dir.path())?;
            let plain = super::machine_key::decode_and_decrypt_v2(&old_key, encoded)?;
            let blob = super::machine_key::encrypt_with_machine_key_v2(&new_key, plain.as_bytes())?;
            *field = format!("{}{}", super::STREAM_KEY_PREFIX_V2, BASE64.encode(&blob));
        }
        Ok(())
    })
    .unwrap();
    crate::services::write_owner_only_atomic(
        &profiles_dir.join("p.json"),
        serde_json::to_string_pretty(&profile).unwrap().as_bytes(),
    )
    .unwrap();
    crate::services::write_owner_only_atomic(
        &data_dir.path().join(".stream_key.new"),
        &*new_key,
    )
    .unwrap();
    std::fs::write(data_dir.path().join(".stream_key"), b"").unwrap();

    let outcome = Encryption::recover_interrupted_rotation(data_dir.path()).unwrap();
    assert_eq!(outcome, RotationRecovery::Promoted);
    assert!(!data_dir.path().join(".stream_key.new").exists());
    assert_eq!(
        std::fs::read(data_dir.path().join(".stream_key")).unwrap(),
        vec![9u8; KEY_LEN]
    );

    // Profile decrypts under the promoted key.
    assert_all_secrets_intact(&mgr.load_with_key_decryption("p", None).await.unwrap());
}

#[test]
fn recover_is_a_no_op_on_clean_installs() {
    let data_dir = TempDir::new().unwrap();
    assert_eq!(
        Encryption::recover_interrupted_rotation(data_dir.path()).unwrap(),
        RotationRecovery::Clean
    );
}
