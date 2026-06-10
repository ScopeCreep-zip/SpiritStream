pub(crate) fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Refuse to start in cloud mode without the safety net.
///
/// Three preconditions are checked, all load-bearing for a public
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
/// 3. **No CORS wildcard** (H1): `SPIRITSTREAM_CORS_ORIGINS` must not
///    contain `*`. A wildcard in cloud mode lets any origin send
///    credentialed requests, defeating the same-origin guard that the
///    Sec-Fetch-Site CSRF check leans on. Explicit allow-list only.
///
/// `pre_deploy_mode == "cloud"` is the only trigger; localhost dev
/// and `desktop` mode never hit this path.
pub(crate) fn enforce_cloud_mode_preconditions(
    auth_token: &Option<String>,
    tls_declared: bool,
    trusted_proxies_configured: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    enforce_cloud_mode_preconditions_with_origins(
        auth_token,
        tls_declared,
        std::env::var("SPIRITSTREAM_CORS_ORIGINS").ok().as_deref(),
        trusted_proxies_configured,
    )
}

/// Pure inner used by tests. Takes the CORS env value explicitly so
/// parallel test runs don't trip on shared global env mutation.
pub(crate) fn enforce_cloud_mode_preconditions_with_origins(
    auth_token: &Option<String>,
    tls_declared: bool,
    cors_origins: Option<&str>,
    trusted_proxies_configured: bool,
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
    if let Some(value) = cors_origins {
        if value.split(',').any(|entry| entry.trim() == "*") {
            return Err("SPIRITSTREAM_DEPLOY_MODE=cloud refuses to start: \
                 SPIRITSTREAM_CORS_ORIGINS contains '*'. Wildcard CORS in cloud \
                 mode lets any browser origin issue credentialed requests, \
                 bypassing the Sec-Fetch-Site CSRF guard. Set an explicit \
                 allow-list of origins instead."
                .into());
        }
    }
    if !trusted_proxies_configured {
        return Err("SPIRITSTREAM_DEPLOY_MODE=cloud refuses to start: \
             SPIRITSTREAM_TRUSTED_PROXIES is unset. Cloud mode requires a \
             TLS-terminating reverse proxy, so the direct peer the server \
             sees is always the proxy — without its CIDR configured, the \
             per-IP login rate limit collapses into ONE shared bucket and \
             any single client can 429-lock login for everyone. Set it to \
             the proxy's address(es), e.g. \
             SPIRITSTREAM_TRUSTED_PROXIES=172.18.0.0/16."
            .into());
    }
    log::info!(
        "Cloud-mode preconditions satisfied: strong API token + TLS-fronted + explicit CORS allow-list + trusted proxies."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strong_token() -> Option<String> {
        Some("0123456789abcdef0123456789abcdef".into())
    }

    #[test]
    fn cloud_mode_refuses_cors_wildcard() {
        let err = enforce_cloud_mode_preconditions_with_origins(
            &strong_token(),
            true,
            Some("https://app.example.com, * , https://other.example.com"),
            true,
        )
        .expect_err("wildcard CORS must be refused in cloud mode");
        let msg = err.to_string();
        assert!(
            msg.contains("'*'"),
            "error must call out the wildcard offender: {msg}",
        );
    }

    #[test]
    fn cloud_mode_accepts_explicit_origin_list() {
        let result = enforce_cloud_mode_preconditions_with_origins(
            &strong_token(),
            true,
            Some("https://app.example.com,https://admin.example.com"),
            true,
        );
        assert!(
            result.is_ok(),
            "explicit origins must be accepted: {result:?}"
        );
    }

    #[test]
    fn cloud_mode_accepts_unset_origins() {
        // No SPIRITSTREAM_CORS_ORIGINS at all is fine — CORS layer
        // defaults to no allow-list, which is the most restrictive
        // posture available.
        let result =
            enforce_cloud_mode_preconditions_with_origins(&strong_token(), true, None, true);
        assert!(result.is_ok());
    }

    /// Behind the (mandatory) TLS proxy, every direct peer is the proxy
    /// itself — without its CIDR configured the login limiter is one
    /// shared bucket. Cloud mode must refuse to start that way.
    #[test]
    fn cloud_mode_requires_trusted_proxies() {
        let err = enforce_cloud_mode_preconditions_with_origins(&strong_token(), true, None, false)
            .expect_err("cloud mode without trusted proxies must refuse startup");
        assert!(err.to_string().contains("SPIRITSTREAM_TRUSTED_PROXIES"));
    }
}
