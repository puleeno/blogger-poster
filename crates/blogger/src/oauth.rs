use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::error::{Error, Result};

pub const OAUTH_SCOPE: &str = "https://www.googleapis.com/auth/blogger";
pub const AUTH_HOST: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenPair {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// unix epoch seconds when the access token expires.
    pub expires_at: u64,
    #[serde(default)]
    pub scope: Option<String>,
}

impl OAuthConfig {
    pub fn authorize_url(&self, state: &str) -> String {
        let params = [
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", OAUTH_SCOPE),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state),
        ];
        let qs = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("{AUTH_HOST}?{qs}")
    }

    pub fn is_valid(&self) -> bool {
        !self.client_id.is_empty() && !self.client_secret.is_empty()
    }
}

impl TokenPair {
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now + 60 >= self.expires_at
    }
}

/// Exchange an authorization code for a token pair.
pub async fn exchange_code(
    http: &reqwest::Client,
    config: &OAuthConfig,
    code: &str,
) -> Result<TokenPair> {
    let res = http
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("code", code.to_string()),
            ("client_id", config.client_id.clone()),
            ("client_secret", config.client_secret.clone()),
            ("redirect_uri", config.redirect_uri.clone()),
            ("grant_type", "authorization_code".to_string()),
        ])
        .send()
        .await?;
    let status = res.status();
    let body = res.text().await?;
    if !status.is_success() {
        return Err(Error::Api { status: status.as_u16(), body });
    }
    let v: serde_json::Value = serde_json::from_str(&body)?;
    build_token(v)
}

/// Refresh an expiring access token using its refresh token.
pub async fn refresh_token(
    http: &reqwest::Client,
    config: &OAuthConfig,
    refresh_token: &str,
) -> Result<TokenPair> {
    let res = http
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", config.client_id.clone()),
            ("client_secret", config.client_secret.clone()),
            ("refresh_token", refresh_token.to_string()),
            ("grant_type", "refresh_token".to_string()),
        ])
        .send()
        .await?;
    let status = res.status();
    let body = res.text().await?;
    if !status.is_success() {
        return Err(Error::Api { status: status.as_u16(), body });
    }
    let v: serde_json::Value = serde_json::from_str(&body)?;
    Ok(TokenPair {
        access_token: v["access_token"].as_str().unwrap_or_default().to_string(),
        refresh_token: Some(refresh_token.to_string()),
        expires_at: token_expiry(&v),
        scope: v["scope"].as_str().map(|s| s.to_string()),
    })
}

fn token_expiry(v: &serde_json::Value) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now + v["expires_in"].as_u64().unwrap_or(3600)
}

fn build_token(v: serde_json::Value) -> Result<TokenPair> {
    let access = v
        .get("access_token")
        .and_then(|t| t.as_str())
        .ok_or_else(|| Error::oauth(format!("missing access_token in {}", v)))?
        .to_string();
    Ok(TokenPair {
        access_token: access,
        refresh_token: v.get("refresh_token").and_then(|t| t.as_str()).map(|s| s.to_string()),
        expires_at: token_expiry(&v),
        scope: v.get("scope").and_then(|s| s.as_str()).map(|s| s.to_string()),
    })
}

/// Spin up a one-shot HTTPS-less callback server that captures `code` from the
/// OAuth redirect. Returns (callback_url, code_receiver).
pub async fn start_oauth_listener(
    port: u16,
) -> Result<(String, oneshot::Receiver<String>)> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let (mut socket, _) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                log::error!("oauth callback accept failed: {e}");
                return;
            }
        };
        let mut buf = [0u8; 8192];
        let _ = socket.read(&mut buf).await;
        let request = String::from_utf8_lossy(&buf).to_string();
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        let code = path
            .split_once('?')
            .and_then(|(_, q)| {
                q.split('&')
                    .find_map(|kv| kv.strip_prefix("code="))
                    .map(|c| urlencoding::decode(c).map(|s| s.into_owned()).unwrap_or_default())
            })
            .ok_or_else(|| Error::oauth("no code in callback url"));

        match code {
            Ok(c) => {
                let _ = tx.send(c);
                let body = String::from("<h2>Blogger Poster</h2><p>Connected! You can close this tab.</p>");
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
            Err(e) => {
                log::warn!("oauth callback parse failed: {e}");
                let _ = tx.send(String::new());
                let body = String::from("<h2>Authorization failed</h2>");
                let response = format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        }
    });

    Ok((format!("http://127.0.0.1:{port}/callback"), rx))
}