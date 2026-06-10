/// Session-cookie hardening mode.
///
/// Selected at startup from `SPIRITSTREAM_COOKIE_MODE` (or auto-detected from
/// bind address and `SPIRITSTREAM_DEPLOY_MODE`). Controls the `Secure` and
/// `SameSite` attributes on the auth cookie:
///
/// * `SameOrigin` — Tauri 2 webview or Docker single-tenant. The UI and API
///   share an origin; strictest cookie policy applies.
///   `Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=604800`.
/// * `CrossOrigin` — browser UI served from a different host than the API
///   (e.g. cloud deploy with separate UI domain). `Strict` would drop the
///   cookie on cross-site navigation back to the UI, so `Lax` is used.
///   `Secure; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800`.
/// * `LocalhostDev` — loopback HTTP development. `Secure` is dropped because
///   the browser refuses Secure cookies over plain HTTP, but `HttpOnly` and
///   `SameSite=Strict` still apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionCookieMode {
    SameOrigin,
    CrossOrigin,
    LocalhostDev,
}

impl SessionCookieMode {
    /// Pure decision function — given an optional `SPIRITSTREAM_COOKIE_MODE`
    /// override, the bind host, and the deploy mode, return the cookie
    /// mode. Production callers read the env var once at startup and
    /// pass it in; tests pass it directly to avoid env-mutation races.
    pub(crate) fn detect(host: &str, deploy_mode: Option<&str>, explicit: Option<&str>) -> Self {
        if let Some(explicit) = explicit {
            match explicit.to_ascii_lowercase().as_str() {
                "same_origin" | "same-origin" | "sameorigin" => return Self::SameOrigin,
                "cross_origin" | "cross-origin" | "crossorigin" => return Self::CrossOrigin,
                "localhost_dev" | "localhost-dev" | "localhostdev" | "dev" => {
                    return Self::LocalhostDev
                }
                other => {
                    // A typo'd explicit override silently picking a
                    // different cookie policy is exactly the silent
                    // fallback this project forbids — be loud, then
                    // auto-detect.
                    log::error!(
                        "unrecognised SPIRITSTREAM_COOKIE_MODE {other:?}; falling back to \
                         auto-detect (valid: same-origin, cross-origin, localhost-dev)"
                    );
                }
            }
        }
        if matches!(deploy_mode, Some(m) if m.eq_ignore_ascii_case("cloud")) {
            return Self::CrossOrigin;
        }
        if is_loopback_host(host) {
            return Self::LocalhostDev;
        }
        Self::SameOrigin
    }

    pub(crate) fn secure(self) -> bool {
        !matches!(self, Self::LocalhostDev)
    }

    pub(crate) fn same_site(self) -> tower_cookies::cookie::SameSite {
        match self {
            Self::CrossOrigin => tower_cookies::cookie::SameSite::Lax,
            _ => tower_cookies::cookie::SameSite::Strict,
        }
    }
}

/// `0.0.0.0` is deliberately NOT loopback: it binds every interface,
/// i.e. it is publicly reachable. Treating it as loopback used to put
/// a Docker/LAN deploy into `LocalhostDev` mode and issue the session
/// cookie WITHOUT `Secure` — sniffable in cleartext on a hostile
/// network.
pub(crate) fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost") || host.starts_with("127.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_interfaces_bind_is_not_loopback() {
        assert!(!is_loopback_host("0.0.0.0"));
        // → auto-detect lands on SameOrigin (Secure cookie), not dev mode.
        assert_eq!(
            SessionCookieMode::detect("0.0.0.0", None, None),
            SessionCookieMode::SameOrigin
        );
        assert!(SessionCookieMode::detect("0.0.0.0", None, None).secure());
    }

    #[test]
    fn loopback_binds_stay_dev_mode() {
        for host in ["127.0.0.1", "localhost", "::1", "127.0.0.53"] {
            assert_eq!(
                SessionCookieMode::detect(host, None, None),
                SessionCookieMode::LocalhostDev,
                "{host}"
            );
        }
    }
}
