use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use aes_gcm_siv::Aes256GcmSiv;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::Rng;
use tempfile::TempDir;

use crate::errors::CoreError;

use super::kdf::derive_key;
use super::machine_key::get_or_create_machine_key;
use super::{Encryption, KEY_LEN, NONCE_LEN, SALT_LEN, STREAM_KEY_PREFIX_V1, STREAM_KEY_PREFIX_V2};

// --- Password-based encryption (profile `.mgs` body) -------------------

#[test]
fn password_roundtrip_v2() {
    let plaintext = b"the quick brown fox jumps over the lazy dog";
    let encrypted = Encryption::encrypt(plaintext, "correct horse battery staple").unwrap();
    let decrypted = Encryption::decrypt_v2(&encrypted, "correct horse battery staple").unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn password_wrong_password_yields_password_incorrect() {
    let encrypted = Encryption::encrypt(b"secret", "right").unwrap();
    let err = Encryption::decrypt_v2(&encrypted, "wrong").unwrap_err();
    assert!(matches!(err, CoreError::PasswordIncorrect));
}

#[test]
fn password_v1_legacy_blobs_still_decrypt() {
    // Build a V1 blob by reimplementing the legacy encrypt path (private
    // helper retained only for this test fixture). Confirms a `.mgs`
    // file produced by a pre-Phase-6 install opens cleanly under the
    // new code.
    fn legacy_encrypt_v1(data: &[u8], password: &str) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let salt: [u8; SALT_LEN] = rng.gen();
        let nonce_bytes: [u8; NONCE_LEN] = rng.gen();
        let key = derive_key(password, &salt).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&*key).unwrap();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, data).unwrap();
        let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&salt);
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        out
    }

    let blob = legacy_encrypt_v1(b"legacy data", "password");
    let plain = Encryption::decrypt_v1(&blob, "password").unwrap();
    assert_eq!(plain, b"legacy data");
}

// --- AES-GCM-SIV nonce-misuse-resistance --------------------------------

#[test]
fn gcm_siv_is_deterministic_under_nonce_reuse() {
    // AES-GCM-SIV is deterministic given (key, nonce, plaintext). Two
    // encryptions with identical inputs produce identical ciphertext —
    // an attacker only learns "did these two ciphertexts encrypt the
    // same plaintext", never the key. With vanilla AES-GCM, the same
    // assertion would still hold (output differs only by the random
    // nonce that is part of the input), so we ALSO check that two
    // different plaintexts under the same key+nonce still decrypt
    // correctly, which is the property AES-GCM-SIV guarantees and
    // AES-GCM cannot (forgery is feasible after one reuse).
    let key: [u8; KEY_LEN] = [42u8; KEY_LEN];
    let nonce_bytes: [u8; NONCE_LEN] = [7u8; NONCE_LEN];
    let cipher = Aes256GcmSiv::new_from_slice(&key).unwrap();
    let nonce = aes_gcm_siv::Nonce::from_slice(&nonce_bytes);

    let pt_a = b"plaintext A";
    let pt_b = b"plaintext B (different length)";

    let ct_a1 = cipher.encrypt(nonce, &pt_a[..]).unwrap();
    let ct_a2 = cipher.encrypt(nonce, &pt_a[..]).unwrap();
    assert_eq!(
        ct_a1, ct_a2,
        "GCM-SIV must be deterministic given identical inputs"
    );

    let ct_b = cipher.encrypt(nonce, &pt_b[..]).unwrap();
    assert_ne!(
        ct_a1, ct_b,
        "different plaintexts must produce different ciphertexts"
    );

    let plain_a = cipher.decrypt(nonce, &ct_a1[..]).unwrap();
    let plain_b = cipher.decrypt(nonce, &ct_b[..]).unwrap();
    assert_eq!(plain_a, pt_a);
    assert_eq!(plain_b, pt_b);
}

// --- Machine-key stream key / token ------------------------------------

#[test]
fn stream_key_roundtrip_v2() {
    let dir = TempDir::new().unwrap();
    let plaintext = "live_1234567890_abcdefghijklmnop";
    let encrypted = Encryption::encrypt_stream_key(plaintext, dir.path()).unwrap();
    assert!(encrypted.starts_with(STREAM_KEY_PREFIX_V2));
    assert!(Encryption::is_stream_key_encrypted(&encrypted));

    let decrypted = Encryption::decrypt_stream_key(&encrypted, dir.path()).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn stream_key_v1_legacy_still_decrypts() {
    // Manually produce a V1 stream key blob with the legacy code path
    // and confirm `decrypt_stream_key` reads it transparently.
    let dir = TempDir::new().unwrap();
    let plaintext = "v1-legacy-stream-key";

    let machine_key = get_or_create_machine_key(dir.path()).unwrap();
    let mut rng = rand::thread_rng();
    let nonce_bytes: [u8; NONCE_LEN] = rng.gen();
    let cipher = Aes256Gcm::new_from_slice(&*machine_key).unwrap();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).unwrap();
    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    let v1_encoded = format!("{}{}", STREAM_KEY_PREFIX_V1, BASE64.encode(&combined));

    assert!(Encryption::is_stream_key_encrypted(&v1_encoded));
    let decrypted = Encryption::decrypt_stream_key(&v1_encoded, dir.path()).unwrap();
    assert_eq!(decrypted, plaintext);
}

#[test]
fn mixed_v1_and_v2_stream_keys_in_same_environment() {
    // A single profile may have some stream keys
    // written under V1 (pre-migration) and some under V2 (re-saved).
    // Confirm both decrypt under one machine key without interference.
    let dir = TempDir::new().unwrap();
    let machine_key = get_or_create_machine_key(dir.path()).unwrap();

    // V2 via public API
    let v2 = Encryption::encrypt_stream_key("plain-v2", dir.path()).unwrap();
    assert!(v2.starts_with(STREAM_KEY_PREFIX_V2));

    // V1 via direct construction (legacy producer)
    let mut rng = rand::thread_rng();
    let nonce_bytes: [u8; NONCE_LEN] = rng.gen();
    let cipher = Aes256Gcm::new_from_slice(&*machine_key).unwrap();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, b"plain-v1".as_ref()).unwrap();
    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    let v1 = format!("{}{}", STREAM_KEY_PREFIX_V1, BASE64.encode(&combined));

    assert_eq!(
        Encryption::decrypt_stream_key(&v1, dir.path()).unwrap(),
        "plain-v1"
    );
    assert_eq!(
        Encryption::decrypt_stream_key(&v2, dir.path()).unwrap(),
        "plain-v2"
    );
}

#[test]
fn already_encrypted_passes_through() {
    let dir = TempDir::new().unwrap();
    let v2 = Encryption::encrypt_stream_key("hello", dir.path()).unwrap();
    let again = Encryption::encrypt_stream_key(&v2, dir.path()).unwrap();
    assert_eq!(
        again, v2,
        "re-encrypting an already-encrypted value must be a no-op"
    );

    // V1 must also pass through unchanged when re-encrypted, so the
    // migration boundary is one-way (only `rotate_machine_key` upgrades).
    let v1_like = format!("{}deadbeef", STREAM_KEY_PREFIX_V1);
    let result = Encryption::encrypt_stream_key(&v1_like, dir.path()).unwrap();
    assert_eq!(result, v1_like);
}

#[test]
fn empty_string_passes_through() {
    let dir = TempDir::new().unwrap();
    let result = Encryption::encrypt_stream_key("", dir.path()).unwrap();
    assert_eq!(result, "");
}

// --- Machine-key rotation -----------------------------

#[tokio::test]
async fn rotate_with_encrypted_profile_re_encrypts_stream_keys() {
    use crate::models::{OutputGroup, Platform, Profile, ProfileSettings, RtmpInput, StreamTarget};
    use crate::services::ProfileManager;
    use std::collections::HashMap;

    let data_dir = TempDir::new().unwrap();
    let profiles_dir = data_dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    let mgr = ProfileManager::new(data_dir.path().to_path_buf());

    // Plaintext profile with one machine-key-encrypted stream key.
    let plain_key = "live_plain_1234567890_abcdef";
    let mut og = OutputGroup::new();
    og.id = "g1".into();
    og.name = "G1".into();
    og.stream_targets = vec![StreamTarget {
        id: "t1".into(),
        service: Platform::Twitch,
        name: "Twitch".into(),
        url: "rtmp://localhost/x".into(),
        stream_key: plain_key.into(),
    }];
    let mut plain = Profile {
        id: "plain".into(),
        name: "plain".into(),
        encrypted: false,
        input: RtmpInput {
            input_type: "rtmp".into(),
            bind_address: "127.0.0.1".into(),
            port: 1935,
            application: "live".into(),
        },
        output_groups: vec![og],
        settings: ProfileSettings {
            encrypt_stream_keys: true,
            ..ProfileSettings::default()
        },
        pii_blocklist: vec![],
        pii_fuzzy: false,
        anonymous_logging: true,
        anonymous_salt: String::new(),
    };
    mgr.save_with_key_encryption(&plain, None).await.unwrap();

    // Encrypted (.mgs) profile under password "correct-horse-battery".
    let enc_key = "live_enc_9876543210_zyxwvu";
    plain.id = "enc".into();
    plain.name = "enc".into();
    plain.input.port = 1936;
    plain.output_groups[0].stream_targets[0].stream_key = enc_key.into();
    mgr.save_with_key_encryption(&plain, Some("correct-horse-battery"))
        .await
        .unwrap();

    // Sanity: enc profile is on disk as .mgs, plain as .json.
    assert!(profiles_dir.join("plain.json").exists());
    assert!(profiles_dir.join("enc.mgs").exists());

    // Rotate, supplying the .mgs password.
    let mut passwords = HashMap::new();
    passwords.insert("enc".to_string(), "correct-horse-battery".to_string());
    let report = Encryption::rotate_machine_key(data_dir.path(), &profiles_dir, &passwords)
        .expect("rotation should succeed");
    assert_eq!(report.profiles_updated, 2);
    assert!(report.keys_reencrypted >= 2);

    // Both profiles must still open cleanly under the new key, with the
    // original plaintext stream keys.
    let loaded_plain = mgr.load_with_key_decryption("plain", None).await.unwrap();
    assert_eq!(
        loaded_plain.output_groups[0].stream_targets[0].stream_key,
        plain_key
    );
    let loaded_enc = mgr
        .load_with_key_decryption("enc", Some("correct-horse-battery"))
        .await
        .unwrap();
    assert_eq!(
        loaded_enc.output_groups[0].stream_targets[0].stream_key,
        enc_key
    );
}

#[tokio::test]
async fn rotate_refuses_when_password_missing_for_encrypted_profile() {
    use crate::models::{Profile, ProfileSettings, RtmpInput};
    use crate::services::ProfileManager;
    use std::collections::HashMap;

    let data_dir = TempDir::new().unwrap();
    let profiles_dir = data_dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    let mgr = ProfileManager::new(data_dir.path().to_path_buf());

    let profile = Profile {
        id: "enc".into(),
        name: "enc".into(),
        encrypted: true,
        input: RtmpInput {
            input_type: "rtmp".into(),
            bind_address: "127.0.0.1".into(),
            port: 1935,
            application: "live".into(),
        },
        output_groups: vec![],
        settings: ProfileSettings::default(),
        pii_blocklist: vec![],
        pii_fuzzy: false,
        anonymous_logging: true,
        anonymous_salt: String::new(),
    };
    mgr.save_with_key_encryption(&profile, Some("correct-horse-battery"))
        .await
        .unwrap();

    // Force machine-key creation, then snapshot it so we can confirm
    // it's untouched after the refusal.
    Encryption::get_or_create_machine_key_public(data_dir.path()).unwrap();
    let key_path = data_dir.path().join(".stream_key");
    let key_before = std::fs::read(&key_path).unwrap();

    let err = Encryption::rotate_machine_key(data_dir.path(), &profiles_dir, &HashMap::new())
        .expect_err("rotation must refuse without password for .mgs profile");
    match err {
        CoreError::ValidationFailed { reasons } => {
            assert!(reasons
                .iter()
                .any(|r| r.code.starts_with("missing_password_for_")));
        }
        other => panic!("expected ValidationFailed, got {other:?}"),
    }

    let key_after = std::fs::read(&key_path).unwrap();
    assert_eq!(
        key_before, key_after,
        "old machine key must be untouched after a refused rotation"
    );
}

/// G7 regression: when a per-profile re-encrypt fails mid-rotation,
/// `restore_from_backup` must put every profile back exactly as it
/// was — and the old machine key must still decrypt them. Pre-G7 we
/// had no test for the rollback path; the cleanup chain could silently
/// regress and only fail in production where rolling back matters
/// most. Forces failure by writing a profile whose machine-key
/// envelope is structurally valid (carries the V2 prefix) but holds
/// non-base64 garbage so the decrypt-with-old-key step errors.
#[tokio::test]
async fn rotate_rolls_back_when_re_encrypt_fails_midway() {
    use crate::models::{OutputGroup, Platform, Profile, ProfileSettings, RtmpInput, StreamTarget};
    use crate::services::ProfileManager;
    use std::collections::HashMap;

    let data_dir = TempDir::new().unwrap();
    let profiles_dir = data_dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    let mgr = ProfileManager::new(data_dir.path().to_path_buf());

    // Healthy plaintext profile with a real encrypted stream key.
    let mut og = OutputGroup::new();
    og.id = "g".into();
    og.name = "G".into();
    og.stream_targets = vec![StreamTarget {
        id: "t".into(),
        service: Platform::Twitch,
        name: "Twitch".into(),
        url: "rtmp://localhost/x".into(),
        stream_key: "live_real_1234567890".into(),
    }];
    let good = Profile {
        id: "good".into(),
        name: "good".into(),
        encrypted: false,
        input: RtmpInput {
            input_type: "rtmp".into(),
            bind_address: "127.0.0.1".into(),
            port: 1935,
            application: "live".into(),
        },
        output_groups: vec![og],
        settings: ProfileSettings {
            encrypt_stream_keys: true,
            ..ProfileSettings::default()
        },
        pii_blocklist: vec![],
        pii_fuzzy: false,
        anonymous_logging: true,
        anonymous_salt: String::new(),
    };
    mgr.save_with_key_encryption(&good, None).await.unwrap();

    // Snapshot the healthy file contents + the old machine key so we
    // can verify the rollback leaves everything exactly as it was.
    let good_path = profiles_dir.join("good.json");
    let good_before = std::fs::read(&good_path).unwrap();
    let key_path = data_dir.path().join(".stream_key");
    let key_before = std::fs::read(&key_path).unwrap();

    // Now inject a poisoned plaintext profile alongside the good one.
    // Carries a V2-prefixed but undecryptable stream_key — re-encrypt
    // path will fail on `decode_and_decrypt_v2`, triggering rollback.
    let poison_json = serde_json::json!({
        "id": "poison",
        "name": "poison",
        "encrypted": false,
        "input": {
            "type": "rtmp",
            "bindAddress": "127.0.0.1",
            "port": 1936,
            "application": "live"
        },
        "outputGroups": [{
            "id": "g",
            "name": "G",
            "video": {
                "codec": "libx264",
                "preset": "veryfast",
                "tune": "zerolatency",
                "profile": "high",
                "bitrate": "6000",
                "width": 1920,
                "height": 1080,
                "fps": 30,
                "keyframeInterval": 2,
            },
            "audio": {
                "codec": "aac",
                "bitrate": "160k",
                "sampleRate": 48000,
                "channels": 2
            },
            "streamTargets": [{
                "id": "t",
                "service": "twitch",
                "name": "T",
                "url": "rtmp://localhost/x",
                "streamKey": "ENC2::!!!not-base64!!!"
            }]
        }],
        "settings": serde_json::Value::Object(serde_json::Map::new()),
        "piiBlocklist": [],
        "piiFuzzy": false,
        "anonymousLogging": true,
        "anonymousSalt": ""
    });
    std::fs::write(
        profiles_dir.join("poison.json"),
        serde_json::to_vec_pretty(&poison_json).unwrap(),
    )
    .unwrap();

    // Rotate. Expect Internal error citing rollback.
    let err = Encryption::rotate_machine_key(data_dir.path(), &profiles_dir, &HashMap::new())
        .expect_err("rotation must fail when a profile's encrypted key is unrecoverable");
    match err {
        CoreError::Internal { context } => {
            assert!(
                context.contains("Key rotation failed") || context.contains("rolled back"),
                "unexpected internal error: {context}",
            );
        }
        other => panic!("expected Internal, got {other:?}"),
    }

    // Old machine key must be intact — step 7 (delete old key) never
    // ran because step 6 errored.
    let key_after = std::fs::read(&key_path).unwrap();
    assert_eq!(
        key_before, key_after,
        "rollback must leave the old machine key in place",
    );

    // The healthy profile must be byte-identical to its pre-rotation state.
    let good_after = std::fs::read(&good_path).unwrap();
    assert_eq!(
        good_before, good_after,
        "rollback must restore the healthy profile byte-for-byte",
    );

    // And the original stream_key must still decrypt under the
    // (unchanged) old machine key.
    let loaded = mgr.load_with_key_decryption("good", None).await.unwrap();
    assert_eq!(
        loaded.output_groups[0].stream_targets[0].stream_key,
        "live_real_1234567890",
    );
}

#[tokio::test]
async fn rotate_refuses_on_wrong_password_for_encrypted_profile() {
    use crate::models::{Profile, ProfileSettings, RtmpInput};
    use crate::services::ProfileManager;
    use std::collections::HashMap;

    let data_dir = TempDir::new().unwrap();
    let profiles_dir = data_dir.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    let mgr = ProfileManager::new(data_dir.path().to_path_buf());

    let profile = Profile {
        id: "enc".into(),
        name: "enc".into(),
        encrypted: true,
        input: RtmpInput {
            input_type: "rtmp".into(),
            bind_address: "127.0.0.1".into(),
            port: 1935,
            application: "live".into(),
        },
        output_groups: vec![],
        settings: ProfileSettings::default(),
        pii_blocklist: vec![],
        pii_fuzzy: false,
        anonymous_logging: true,
        anonymous_salt: String::new(),
    };
    mgr.save_with_key_encryption(&profile, Some("correct-horse-battery"))
        .await
        .unwrap();

    Encryption::get_or_create_machine_key_public(data_dir.path()).unwrap();
    let key_path = data_dir.path().join(".stream_key");
    let key_before = std::fs::read(&key_path).unwrap();

    let mut passwords = HashMap::new();
    passwords.insert("enc".to_string(), "wrong-password".to_string());
    let err = Encryption::rotate_machine_key(data_dir.path(), &profiles_dir, &passwords)
        .expect_err("rotation must refuse on wrong password");
    assert!(matches!(err, CoreError::PasswordIncorrect));

    let key_after = std::fs::read(&key_path).unwrap();
    assert_eq!(
        key_before, key_after,
        "old machine key must be untouched after wrong-password refusal"
    );
}
