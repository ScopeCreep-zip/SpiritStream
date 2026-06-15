use super::connection::obs_connection_params_changed;
use super::types::{IntegrationDirection, ObsConfig};
use super::ObsWebSocketHandler;
use tempfile::TempDir;

/// Build a fresh handler in an isolated tempdir. J4: pre-fix the
/// suite hard-coded `/tmp/spiritstream-obs-test`, which (a) is not
/// portable to Windows where `/tmp` doesn't exist, and (b) shared
/// state across parallel test runs on the same machine. `TempDir`
/// gets dropped at scope-end so each test has its own
/// guaranteed-clean directory.
fn handler() -> (TempDir, ObsWebSocketHandler) {
    let dir = TempDir::new().expect("tempdir");
    let h = ObsWebSocketHandler::new(dir.path().to_path_buf());
    (dir, h)
}

/// `consume_triggered_by_us` is the loop-prevention seam: after
/// SpiritStream drives OBS via `mark_triggered_by_us`, the inbound state
/// event must see the flag set exactly once and then false on every
/// subsequent read in the same cycle — otherwise a Bidirectional config
/// loops forever.
#[test]
fn mark_then_consume_returns_true_then_false() {
    let (_dir, h) = handler();
    assert!(
        !h.consume_triggered_by_us(),
        "fresh handler must be unmarked"
    );
    h.mark_triggered_by_us();
    assert!(
        h.consume_triggered_by_us(),
        "first read after mark must observe true"
    );
    assert!(
        !h.consume_triggered_by_us(),
        "second read must atomically have cleared"
    );
}

/// Re-arming the flag must work for back-to-back triggers (the
/// `start_stream` → `stop_stream` sequence within one session).
#[test]
fn mark_consume_mark_consume_rearms() {
    let (_dir, h) = handler();
    h.mark_triggered_by_us();
    assert!(h.consume_triggered_by_us());
    h.mark_triggered_by_us();
    assert!(h.consume_triggered_by_us());
    assert!(!h.consume_triggered_by_us());
}

/// Direction gating: each `IntegrationDirection` variant must permit only
/// the documented trigger paths. Bidirectional permits both; Disabled
/// permits neither. Single-direction variants permit exactly one path.
#[tokio::test]
async fn direction_gates_match_documented_transitions() {
    let (_dir, h) = handler();
    let base = ObsConfig {
        host: "127.0.0.1".into(),
        port: 4455,
        password: String::new(),
        use_auth: false,
        direction: IntegrationDirection::Disabled,
        auto_connect: false,
    };
    h.set_config(base.clone()).await;
    assert!(!h.should_obs_trigger_spiritstream().await);
    assert!(!h.should_spiritstream_trigger_obs().await);

    h.set_config(ObsConfig {
        direction: IntegrationDirection::ObsToSpiritstream,
        ..base.clone()
    })
    .await;
    assert!(h.should_obs_trigger_spiritstream().await);
    assert!(!h.should_spiritstream_trigger_obs().await);

    h.set_config(ObsConfig {
        direction: IntegrationDirection::SpiritstreamToObs,
        ..base.clone()
    })
    .await;
    assert!(!h.should_obs_trigger_spiritstream().await);
    assert!(h.should_spiritstream_trigger_obs().await);

    h.set_config(ObsConfig {
        direction: IntegrationDirection::Bidirectional,
        ..base.clone()
    })
    .await;
    assert!(h.should_obs_trigger_spiritstream().await);
    assert!(h.should_spiritstream_trigger_obs().await);
}

/// `apply_profile_obs` reconnects an already-open socket only when a
/// connection-affecting param changed. This pins the decision helper that
/// drives that: host/port/password/use_auth force a reconnect;
/// direction/auto_connect (cascade + supervisor concerns) do NOT.
#[test]
fn connection_params_change_excludes_direction_and_auto_connect() {
    let base = ObsConfig {
        host: "localhost".into(),
        port: 4455,
        password: String::new(),
        use_auth: false,
        direction: IntegrationDirection::Disabled,
        auto_connect: false,
    };

    // Identical → no reconnect.
    assert!(!obs_connection_params_changed(&base, &base.clone()));

    // Direction / auto_connect changes → no reconnect (handled live).
    assert!(!obs_connection_params_changed(
        &base,
        &ObsConfig {
            direction: IntegrationDirection::Bidirectional,
            ..base.clone()
        }
    ));
    assert!(!obs_connection_params_changed(
        &base,
        &ObsConfig {
            auto_connect: true,
            ..base.clone()
        }
    ));

    // Connection params → reconnect required.
    for changed in [
        ObsConfig {
            host: "obs.local".into(),
            ..base.clone()
        },
        ObsConfig {
            port: 4444,
            ..base.clone()
        },
        ObsConfig {
            password: "pw".into(),
            ..base.clone()
        },
        ObsConfig {
            use_auth: true,
            ..base.clone()
        },
    ] {
        assert!(obs_connection_params_changed(&base, &changed));
    }
}

/// Loop scenario: simulate `start_stream` having flipped the flag (the
/// real code path goes through OBS's RPC; this asserts the loop-prevention
/// contract independently of the network IO). The orchestration layer's
/// rule is: when an inbound state event lands with the flag set, that
/// event was caused by us and must NOT bounce back through the trigger
/// path; subsequent state events (no flag) proceed normally.
#[test]
fn loop_scenario_first_event_skips_trigger() {
    let (_dir, h) = handler();
    h.mark_triggered_by_us();
    let was_self = h.consume_triggered_by_us();
    assert!(
        was_self,
        "the post-mark event must be classified as self-triggered"
    );
    let was_self_again = h.consume_triggered_by_us();
    assert!(
        !was_self_again,
        "subsequent events must not be classified as self-triggered"
    );
}
