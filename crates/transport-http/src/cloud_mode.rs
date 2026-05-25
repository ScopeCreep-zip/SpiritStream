pub(crate) fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Refuse to start in cloud mode without the safety net.
///
/// Two preconditions are checked, both load-bearing for a public
/// deployment:
///
/// 1. **Strong API token**: `SPIRITSTREAM_API_TOKEN` (or, falling back,
///    a profile-stored backend token) must be present and at least 32
///    characters. Anything weaker is brute-force-trivial over the
///    public internet even with the lockout in place.
/// 2. **TLS in front**: the operator must set
///    `SPIRITSTREAM_BEHIND_TLS_PROXY=1` to declare that a reverse
///    proxy (Caddy / Traefik / nginx) terminates TLS in front of the
///    server. Cloud-mode HTTP-only is never acceptable — bearer
///    tokens and session cookies would flow in plaintext.
///
/// `pre_deploy_mode == "cloud"` is the only trigger; localhost dev
/// and `desktop` mode never hit this path.
pub(crate) fn enforce_cloud_mode_preconditions(
    auth_token: &Option<String>,
    tls_declared: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    const MIN_TOKEN_LEN: usize = 32;
    let token_ok = auth_token
        .as_deref()
        .map(|t| t.len() >= MIN_TOKEN_LEN)
        .unwrap_or(false);
    if !token_ok {
        return Err(format!(
            "SPIRITSTREAM_DEPLOY_MODE=cloud refuses to start: \
             SPIRITSTREAM_API_TOKEN must be ≥ {MIN_TOKEN_LEN} characters. \
             Generate one with `openssl rand -base64 32`."
        )
        .into());
    }
    if !tls_declared {
        return Err("SPIRITSTREAM_DEPLOY_MODE=cloud refuses to start: \
             set SPIRITSTREAM_BEHIND_TLS_PROXY=1 once a TLS-terminating \
             reverse proxy (Caddy / Traefik / nginx) is in place. \
             Cloud deployments without TLS leak session tokens in cleartext."
            .into());
    }
    log::info!("Cloud-mode preconditions satisfied: strong API token + TLS-fronted declared.");
    Ok(())
}
