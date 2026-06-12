use crate::errors::CoreError;
use log::{error, info, warn};
use std::collections::HashMap;
use tokio::sync::oneshot;

/// Callback server for handling OAuth redirects on the loopback redirect URI.
pub struct OAuthCallbackServer {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl OAuthCallbackServer {
    /// Start a callback server on the specified port.
    pub async fn start(port: u16) -> Result<(Self, oneshot::Receiver<OAuthCallback>), CoreError> {
        let (callback_tx, callback_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let addr = format!("127.0.0.1:{}", port);

        tokio::spawn(async move {
            Self::run_server(&addr, callback_tx, shutdown_rx).await;
        });

        Ok((
            Self {
                shutdown_tx: Some(shutdown_tx),
            },
            callback_rx,
        ))
    }

    async fn run_server(
        addr: &str,
        callback_tx: oneshot::Sender<OAuthCallback>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind OAuth callback server: {}", e);
                return;
            }
        };

        info!("OAuth callback server listening on {}", addr);

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    info!("OAuth callback server shutting down");
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((mut socket, _)) => {
                            let mut buffer = vec![0u8; 4096];
                            if let Ok(n) = socket.read(&mut buffer).await {
                                let request = String::from_utf8_lossy(&buffer[..n]);

                                if let Some(callback) = Self::parse_callback(&request) {
                                    let response = Self::success_response();
                                    let _ = socket.write_all(response.as_bytes()).await;

                                    let _ = callback_tx.send(callback);
                                    break;
                                } else {
                                    // Every flow is an authorization-code grant
                                    // now — a callback without code/state query
                                    // params is malformed, full stop. (The old
                                    // implicit grant's fragment-extraction HTML
                                    // lived here; it died with the implicit flow.)
                                    let response = Self::error_response("Invalid callback");
                                    let _ = socket.write_all(response.as_bytes()).await;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to accept connection: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn parse_callback(request: &str) -> Option<OAuthCallback> {
        let first_line = request.lines().next()?;
        let path = first_line.split_whitespace().nth(1)?;

        if !path.starts_with("/oauth/callback") {
            return None;
        }

        let query = path.split('?').nth(1)?;

        let params: HashMap<&str, &str> = query
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.split('=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        // H5: every `urlencoding::decode().unwrap_or_default()` here
        // silently swallowed malformed input and presented the
        // empty string downstream as if it were a valid OAuth value.
        // A flipped bit on the wire or a curiosity-driven `%ZZ` from
        // a wrong-client redirect would then trip the token exchange
        // with a blank `code`, producing a generic upstream 4xx with
        // no breadcrumb pointing at the corruption. Now decode
        // failures abort the callback parse — the loopback server
        // returns the no-match fallback page rather than confidently
        // forwarding garbage.
        if let Some(error) = params.get("error") {
            let decoded_error = urlencoding::decode(error).ok()?.to_string();
            let description = match params.get("error_description") {
                Some(d) => Some(urlencoding::decode(d).ok()?.to_string()),
                None => None,
            };
            return Some(OAuthCallback::Error {
                error: decoded_error,
                description,
            });
        }

        let code = params.get("code")?;
        let state = params.get("state")?;

        Some(OAuthCallback::Success {
            code: urlencoding::decode(code).ok()?.to_string(),
            state: urlencoding::decode(state).ok()?.to_string(),
        })
    }

    fn success_response() -> String {
        let body = r#"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Successful</title>
    <style>
        body { font-family: system-ui; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e; color: #eee; }
        .container { text-align: center; }
        h1 { color: #a78bfa; }
        p { color: #9ca3af; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Authentication Successful</h1>
        <p>You can close this window and return to SpiritStream.</p>
        <script>setTimeout(() => window.close(), 3000);</script>
    </div>
</body>
</html>"#;

        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    fn error_response(message: &str) -> String {
        let body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>Authentication Failed</title>
    <style>
        body {{ font-family: system-ui; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e; color: #eee; }}
        .container {{ text-align: center; }}
        h1 {{ color: #ef4444; }}
        p {{ color: #9ca3af; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Authentication Failed</h1>
        <p>{}</p>
    </div>
</body>
</html>"#,
            message
        );

        format!(
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
    }

    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// OAuth callback result.
#[derive(Debug, Clone)]
pub enum OAuthCallback {
    /// Authorization code flow callback.
    Success { code: String, state: String },
    Error {
        error: String,
        description: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::{OAuthCallback, OAuthCallbackServer};

    fn parse(line: &str) -> Option<OAuthCallback> {
        OAuthCallbackServer::parse_callback(line)
    }

    #[test]
    fn parses_authorization_code_callback() {
        let req = "GET /oauth/callback?code=abc123&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        match parse(req) {
            Some(OAuthCallback::Success { code, state }) => {
                assert_eq!(code, "abc123");
                assert_eq!(state, "xyz");
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn url_decodes_code_and_state() {
        let req = "GET /oauth/callback?code=a%20b&state=x%2By HTTP/1.1\r\n\r\n";
        match parse(req) {
            Some(OAuthCallback::Success { code, state }) => {
                assert_eq!(code, "a b");
                assert_eq!(state, "x+y");
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn queryless_callback_is_rejected_not_treated_as_implicit() {
        // The implicit grant is gone: a callback with no query params is
        // malformed, never an invitation to serve fragment-hoisting HTML.
        let req = "GET /oauth/callback HTTP/1.1\r\n\r\n";
        assert!(parse(req).is_none());
    }

    #[test]
    fn parses_error_callback_with_description() {
        let req = "GET /oauth/callback?error=access_denied&error_description=nope HTTP/1.1\r\n\r\n";
        match parse(req) {
            Some(OAuthCallback::Error { error, description }) => {
                assert_eq!(error, "access_denied");
                assert_eq!(description.as_deref(), Some("nope"));
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn parses_error_callback_without_description() {
        let req = "GET /oauth/callback?error=server_error HTTP/1.1\r\n\r\n";
        match parse(req) {
            Some(OAuthCallback::Error { error, description }) => {
                assert_eq!(error, "server_error");
                assert!(description.is_none());
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn non_callback_path_returns_none() {
        assert!(parse("GET /favicon.ico HTTP/1.1\r\n\r\n").is_none());
    }

    #[test]
    fn callback_without_query_returns_none() {
        // No query string → implicit-flow fragment page is served instead.
        assert!(parse("GET /oauth/callback HTTP/1.1\r\n\r\n").is_none());
    }

    #[test]
    fn malformed_percent_encoding_aborts_parse() {
        // H5: a percent-escape that decodes to invalid UTF-8 (0xFF) must
        // abort the parse (return None) rather than silently forwarding an
        // empty value downstream.
        assert!(parse("GET /oauth/callback?code=%FF&state=ok HTTP/1.1\r\n\r\n").is_none());
    }

    #[test]
    fn empty_request_returns_none() {
        assert!(parse("").is_none());
    }

    #[test]
    fn success_response_is_well_formed_http() {
        let resp = OAuthCallbackServer::success_response();
        assert!(resp.starts_with("HTTP/1.1 200 OK"));
        assert!(resp.contains("Authentication Successful"));
        assert!(resp.contains("Content-Length:"));
    }

    #[test]
    fn error_response_carries_message_and_400() {
        let resp = OAuthCallbackServer::error_response("boom");
        assert!(resp.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(resp.contains("boom"));
    }

}
