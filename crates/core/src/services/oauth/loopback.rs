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
                                    // No query params — might be implicit flow with token in fragment.
                                    let first_line = request.lines().next().unwrap_or("");
                                    let path = first_line.split_whitespace().nth(1).unwrap_or("");
                                    if path.starts_with("/oauth/callback") {
                                        // Serve HTML that extracts fragment and redirects with query params.
                                        let response = Self::fragment_extraction_response();
                                        let _ = socket.write_all(response.as_bytes()).await;
                                        // Don't break — wait for the second request with query params.
                                    } else {
                                        let response = Self::error_response("Invalid callback");
                                        let _ = socket.write_all(response.as_bytes()).await;
                                    }
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

        let query = match path.split('?').nth(1) {
            Some(q) => q,
            None => {
                // No query params — implicit flow redirect with the token
                // in the URL fragment (client-side only). Return None so
                // the callback server serves the fragment-extraction HTML.
                return None;
            }
        };

        let params: HashMap<&str, &str> = query
            .split('&')
            .filter_map(|pair| {
                let mut parts = pair.split('=');
                Some((parts.next()?, parts.next()?))
            })
            .collect();

        if let Some(error) = params.get("error") {
            return Some(OAuthCallback::Error {
                error: urlencoding::decode(error).unwrap_or_default().to_string(),
                description: params
                    .get("error_description")
                    .map(|d| urlencoding::decode(d).unwrap_or_default().to_string()),
            });
        }

        if let Some(access_token) = params.get("access_token") {
            let state = params.get("state")?;
            return Some(OAuthCallback::ImplicitSuccess {
                access_token: urlencoding::decode(access_token)
                    .unwrap_or_default()
                    .to_string(),
                state: urlencoding::decode(state).unwrap_or_default().to_string(),
            });
        }

        let code = params.get("code")?;
        let state = params.get("state")?;

        Some(OAuthCallback::Success {
            code: urlencoding::decode(code).unwrap_or_default().to_string(),
            state: urlencoding::decode(state).unwrap_or_default().to_string(),
        })
    }

    /// HTML page that extracts OAuth token from URL fragment (implicit flow)
    /// and redirects to the same URL with the fragment as query parameters.
    fn fragment_extraction_response() -> String {
        let body = r#"<!DOCTYPE html>
<html>
<head>
    <title>Processing Authentication...</title>
    <style>
        body { font-family: system-ui; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e; color: #eee; }
        .container { text-align: center; }
        h1 { color: #a78bfa; }
        p { color: #9ca3af; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Processing...</h1>
        <p>Completing authentication, please wait.</p>
    </div>
    <script>
        // The OAuth token is in the URL fragment (#access_token=...)
        // Fragments aren't sent to the server, so we redirect with them as query params
        if (window.location.hash) {
            var params = window.location.hash.substring(1);
            window.location.replace('/oauth/callback?' + params);
        } else {
            document.querySelector('h1').textContent = 'Authentication Failed';
            document.querySelector('p').textContent = 'No authentication data received.';
        }
    </script>
</body>
</html>"#;

        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
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
    /// Authorization code flow callback (YouTube, etc.).
    Success { code: String, state: String },
    /// Implicit flow callback — token arrives directly in the
    /// redirect URL (Twitch implicit grant; fragment hoisted into the
    /// query by the callback HTML before SpiritStream sees it).
    ImplicitSuccess {
        access_token: String,
        state: String,
    },
    Error {
        error: String,
        description: Option<String>,
    },
}
