//! Trovo Open Platform OAuth — deliberately its own module because
//! Trovo's token endpoints deviate from RFC 6749 form-encoding in
//! three ways: requests are JSON bodies with a `client-id` HEADER,
//! authenticated calls use `Authorization: OAuth <token>` (not
//! Bearer), and `expires_in` comes back as a STRING ("14400").
//! Shapes per developer.trovo.live (APIs.html, verified 2026-06).

use serde::Deserialize;

use super::{network, OAuthTokens};
use crate::errors::CoreError;

/// Trovo token response — `expires_in` is a string on the wire.
#[derive(Deserialize)]
struct TrovoTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
}

impl From<TrovoTokenResponse> for OAuthTokens {
    fn from(r: TrovoTokenResponse) -> Self {
        OAuthTokens {
            access_token: r.access_token,
            refresh_token: r.refresh_token,
            expires_in: r.expires_in.and_then(|s| s.parse().ok()),
            token_type: r.token_type,
            scope: None,
        }
    }
}

/// `GET /openplatform/getuserinfo` response (scope `user_details_self`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrovoUser {
    pub user_id: String,
    pub user_name: String,
    #[serde(default)]
    pub nick_name: String,
}

impl super::OAuthService {
    /// Exchange an authorization code at Trovo's `exchangetoken`.
    pub(in crate::services::oauth) async fn exchange_trovo_code(
        &self,
        token_url: &str,
        client_id: &str,
        client_secret: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<OAuthTokens, CoreError> {
        let body = serde_json::json!({
            "client_secret": client_secret,
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": redirect_uri,
        });
        self.trovo_token_request(token_url, client_id, &body).await
    }

    /// Refresh at Trovo's `refreshtoken`.
    pub(in crate::services::oauth) async fn refresh_trovo_token(
        &self,
        refresh_url: &str,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> Result<OAuthTokens, CoreError> {
        let body = serde_json::json!({
            "client_secret": client_secret,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
        });
        self.trovo_token_request(refresh_url, client_id, &body).await
    }

    async fn trovo_token_request(
        &self,
        url: &str,
        client_id: &str,
        body: &serde_json::Value,
    ) -> Result<OAuthTokens, CoreError> {
        let response = self
            .http_client
            .post(url)
            .header("Accept", "application/json")
            .header("client-id", client_id)
            .json(body)
            .send()
            .await
            .map_err(|e| network(format!("Trovo token request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            // Body dropped without logging — it echoes the submitted
            // client_secret / refresh_token on errors.
            let _ = response.text().await;
            return if status.is_client_error() {
                Err(CoreError::Unauthorized)
            } else {
                Err(CoreError::NetworkError {
                    detail: format!("Trovo token request failed: {status}"),
                })
            };
        }
        let parsed: TrovoTokenResponse = response
            .json()
            .await
            .map_err(|e| network(format!("Trovo token parse failed: {e}")))?;
        Ok(parsed.into())
    }

    /// Bearer-identified Trovo user (`Authorization: OAuth <token>`).
    pub(in crate::services::oauth) async fn fetch_trovo_user(
        &self,
        access_token: &str,
    ) -> Result<TrovoUser, CoreError> {
        let provider = self.provider_for("trovo")?;
        let client_id = self.client_id_for("trovo").await;
        let response = self
            .http_client
            .get(&provider.user_info_url)
            .header("Accept", "application/json")
            .header("Client-ID", client_id)
            .header("Authorization", format!("OAuth {access_token}"))
            .send()
            .await
            .map_err(|e| network(format!("Trovo user request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let _ = response.text().await;
            log::error!("Trovo user fetch failed: {status}");
            return Err(CoreError::Unauthorized);
        }
        response
            .json::<TrovoUser>()
            .await
            .map_err(|e| network(format!("Trovo user parse failed: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{OAuthConfig, OAuthProvider, OAuthService};
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn trovo_svc(mock_base: &str) -> OAuthService {
        let svc = OAuthService::new(OAuthConfig {
            trovo_client_id: Some("test-trovo-id".into()),
            trovo_client_secret: Some("test-trovo-secret".into()),
            ..OAuthConfig::default()
        });
        svc.override_provider(OAuthProvider {
            name: "trovo".into(),
            auth_url: format!("{mock_base}/page/login.html"),
            token_url: format!("{mock_base}/openplatform/exchangetoken"),
            device_url: None,
            refresh_url: Some(format!("{mock_base}/openplatform/refreshtoken")),
            user_info_url: format!("{mock_base}/openplatform/getuserinfo"),
            scopes: vec!["user_details_self", "chat_send_self"],
        });
        svc
    }

    #[tokio::test]
    async fn trovo_exchange_sends_json_with_client_id_header_and_parses_string_expiry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openplatform/exchangetoken"))
            .and(header("client-id", "test-trovo-id"))
            .and(header("content-type", "application/json"))
            .and(body_string_contains("\"grant_type\":\"authorization_code\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "trovo-access",
                "refresh_token": "trovo-refresh",
                "expires_in": "14400",
                "token_type": "bearer"
            })))
            .mount(&server)
            .await;

        let svc = trovo_svc(&server.uri());
        let tokens = svc
            .exchange_trovo_code(
                &format!("{}/openplatform/exchangetoken", server.uri()),
                "test-trovo-id",
                "test-trovo-secret",
                "auth-code",
                "http://localhost:8891/oauth/callback",
            )
            .await
            .expect("trovo exchange");
        assert_eq!(tokens.access_token, "trovo-access");
        // Trovo's string "14400" parses into the numeric field.
        assert_eq!(tokens.expires_in, Some(14400));
    }

    #[tokio::test]
    async fn trovo_user_fetch_uses_oauth_authorization_scheme() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/openplatform/getuserinfo"))
            .and(header("authorization", "OAuth trovo-access"))
            .and(header("client-id", "test-trovo-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "userId": "777",
                "userName": "trovostreamer",
                "nickName": "Trovo Streamer"
            })))
            .mount(&server)
            .await;

        let svc = trovo_svc(&server.uri());
        let user = svc.fetch_trovo_user("trovo-access").await.expect("user");
        assert_eq!(user.user_id, "777");
        assert_eq!(user.user_name, "trovostreamer");
    }

    #[tokio::test]
    async fn trovo_refresh_posts_refresh_grant_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/openplatform/refreshtoken"))
            .and(body_string_contains("\"grant_type\":\"refresh_token\""))
            .and(body_string_contains("\"refresh_token\":\"old-rt\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "fresh-access",
                "refresh_token": "fresh-rt",
                "expires_in": "14400"
            })))
            .mount(&server)
            .await;

        let svc = trovo_svc(&server.uri());
        let tokens = svc
            .refresh_trovo_token(
                &format!("{}/openplatform/refreshtoken", server.uri()),
                "test-trovo-id",
                "test-trovo-secret",
                "old-rt",
            )
            .await
            .expect("trovo refresh");
        assert_eq!(tokens.access_token, "fresh-access");
        assert_eq!(tokens.refresh_token.as_deref(), Some("fresh-rt"));
    }
}
