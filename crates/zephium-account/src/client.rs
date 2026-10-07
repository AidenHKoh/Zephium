//! HTTP calls to the account service. Bodies are bounded, redirects refused,
//! and nothing a server says is logged or passed on as text.

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, RETRY_AFTER};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use url::Url;

use crate::wire::{self, Billing, ClientConfig, Profile, Purchase, Session, Usage};
use crate::{AccountError, AccountToken};

const MAX_BODY_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const TIMEOUT: Duration = Duration::from_secs(25);
const MAX_RETRY_AFTER: u32 = 15 * 60;
const DEVICE_HEADER: &str = "x-zephium-device";
const TOKEN_HEADER: &str = "set-auth-token";
/// Checkout and the billing portal must open on the billing provider itself.
const BILLING_HOSTS: [&str; 2] = ["polar.sh", "sandbox.polar.sh"];

/// The account service's origin: https, or plain http on loopback for a
/// server running on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin(Url);

impl Origin {
    pub fn parse(text: &str) -> Result<Self, AccountError> {
        let url = Url::parse(text.trim()).map_err(|_| AccountError::Invalid)?;
        let loopback = matches!(
            url.host(),
            Some(url::Host::Domain("localhost"))
                | Some(url::Host::Ipv4(std::net::Ipv4Addr::LOCALHOST))
                | Some(url::Host::Ipv6(std::net::Ipv6Addr::LOCALHOST))
        );
        let scheme_ok = url.scheme() == "https" || (url.scheme() == "http" && loopback);
        if !scheme_ok
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(AccountError::Invalid);
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str().trim_end_matches('/')
    }

    fn join(&self, path: &str) -> Url {
        let mut url = self.0.clone();
        url.set_path(path);
        url
    }
}

pub struct Client {
    origin: Origin,
    http: reqwest::Client,
}

impl Client {
    pub fn new(origin: Origin) -> Result<Self, AccountError> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TIMEOUT)
            .https_only(origin.0.scheme() == "https")
            .build()
            .map_err(|_| AccountError::Unavailable)?;
        Ok(Self { origin, http })
    }

    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    pub async fn client_config(&self) -> Result<ClientConfig, AccountError> {
        self.json(Method::GET, "/v1/client-config", None, None)
            .await
    }

    pub async fn send_code(&self, email: &str) -> Result<(), AccountError> {
        let body = json!({ "email": email, "type": "sign-in" });
        self.call(
            Method::POST,
            "/api/auth/email-otp/send-verification-otp",
            None,
            Some(body),
        )
        .await
        .map(drop)
    }

    pub async fn verify_code(
        &self,
        email: &str,
        code: &str,
        device: &str,
    ) -> Result<AccountToken, AccountError> {
        let body = json!({ "email": email, "otp": code });
        let mut headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(device) {
            headers.insert(DEVICE_HEADER, value);
        }
        let reply = self
            .request(
                Method::POST,
                "/api/auth/sign-in/email-otp",
                None,
                Some(body),
                headers,
            )
            .await?;
        let token = reply
            .headers
            .get(TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .ok_or(AccountError::Protocol)?;
        AccountToken::new(token.to_owned())
    }

    pub async fn profile(&self, token: &AccountToken) -> Result<Profile, AccountError> {
        self.json(Method::GET, "/v1/account", Some(token), None)
            .await
    }

    pub async fn sessions(&self, token: &AccountToken) -> Result<Vec<Session>, AccountError> {
        let sessions: wire::Sessions = self
            .json(Method::GET, "/v1/sessions", Some(token), None)
            .await?;
        Ok(sessions.sessions)
    }

    pub async fn revoke_session(&self, token: &AccountToken, id: &str) -> Result<(), AccountError> {
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(AccountError::Invalid);
        }
        let path = format!("/v1/sessions/{id}");
        self.call(Method::DELETE, &path, Some(token), None)
            .await
            .map(drop)
    }

    pub async fn sign_out(&self, token: &AccountToken) -> Result<(), AccountError> {
        self.call(Method::POST, "/v1/logout", Some(token), None)
            .await
            .map(drop)
    }

    pub async fn billing(&self, token: &AccountToken) -> Result<Billing, AccountError> {
        self.json(Method::GET, "/v1/billing", Some(token), None)
            .await
    }

    pub async fn refresh_billing(&self, token: &AccountToken) -> Result<Billing, AccountError> {
        self.json(Method::POST, "/v1/billing/refresh", Some(token), None)
            .await
    }

    pub async fn usage(&self, token: &AccountToken) -> Result<Usage, AccountError> {
        self.json(Method::GET, "/v1/usage", Some(token), None).await
    }

    pub async fn checkout(
        &self,
        token: &AccountToken,
        purchase: &Purchase,
    ) -> Result<Url, AccountError> {
        let body = match purchase {
            Purchase::Plan { plan, interval } => json!({ "plan": plan, "interval": interval }),
            Purchase::TopUp { id } => json!({ "topup": id }),
        };
        let link: wire::Link = self
            .json(
                Method::POST,
                "/v1/billing/checkout",
                Some(token),
                Some(body),
            )
            .await?;
        billing_link(&link.url)
    }

    pub async fn portal(&self, token: &AccountToken) -> Result<Url, AccountError> {
        let link: wire::Link = self
            .json(Method::POST, "/v1/billing/portal", Some(token), None)
            .await?;
        billing_link(&link.url)
    }

    pub async fn send_deletion_code(&self, token: &AccountToken) -> Result<(), AccountError> {
        self.call(Method::POST, "/v1/account/deletion/code", Some(token), None)
            .await
            .map(drop)
    }

    pub async fn delete_account(
        &self,
        token: &AccountToken,
        code: &str,
    ) -> Result<(), AccountError> {
        let body = json!({ "otp": code });
        self.call(
            Method::POST,
            "/v1/account/deletion",
            Some(token),
            Some(body),
        )
        .await
        .map(drop)
    }

    async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        token: Option<&AccountToken>,
        body: Option<Value>,
    ) -> Result<T, AccountError> {
        let reply = self.call(method, path, token, body).await?;
        serde_json::from_slice(&reply.body).map_err(|_| AccountError::Protocol)
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        token: Option<&AccountToken>,
        body: Option<Value>,
    ) -> Result<Reply, AccountError> {
        self.request(method, path, token, body, HeaderMap::new())
            .await
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        token: Option<&AccountToken>,
        body: Option<Value>,
        mut headers: HeaderMap,
    ) -> Result<Reply, AccountError> {
        if let Some(token) = token {
            headers.insert(AUTHORIZATION, token.header()?);
        }
        let mut request = self
            .http
            .request(method, self.origin.join(path))
            .headers(headers);
        if let Some(body) = body {
            let bytes = serde_json::to_vec(&body).map_err(|_| AccountError::Invalid)?;
            request = request.header(CONTENT_TYPE, "application/json").body(bytes);
        }
        let mut response = request.send().await.map_err(|error| {
            if error.is_timeout() || error.is_connect() || error.is_request() {
                AccountError::Offline
            } else {
                AccountError::Protocol
            }
        })?;
        let status = response.status();
        let headers = response.headers().clone();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| AccountError::Offline)? {
            if body.len() + chunk.len() > MAX_BODY_BYTES {
                return Err(AccountError::Protocol);
            }
            body.extend_from_slice(&chunk);
        }
        if status.is_success() {
            return Ok(Reply { headers, body });
        }
        Err(classify(status, &headers, &body))
    }
}

struct Reply {
    headers: HeaderMap,
    body: Vec<u8>,
}

fn billing_link(text: &str) -> Result<Url, AccountError> {
    let url = Url::parse(text).map_err(|_| AccountError::Protocol)?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https" || !BILLING_HOSTS.contains(&host) {
        return Err(AccountError::Protocol);
    }
    Ok(url)
}

fn classify(status: StatusCode, headers: &HeaderMap, body: &[u8]) -> AccountError {
    let code = serde_json::from_slice::<wire::ErrorBody>(body)
        .map(|body| body.error.code)
        .unwrap_or_default();
    match (status.as_u16(), code.as_str()) {
        (_, "invalid_code" | "invalid_otp" | "otp_expired" | "too_many_attempts") => {
            AccountError::InvalidCode
        }
        (_, "signup_closed" | "signup_disabled") => AccountError::SignupClosed,
        (_, "no_billing_account") => AccountError::NoBillingAccount,
        (_, "billing_not_enabled" | "cloud_disabled") => AccountError::Unavailable,
        (401, _) => AccountError::SignedOut,
        (429, _) => AccountError::RateLimited {
            retry_after: retry_after(headers),
        },
        (400 | 404 | 405 | 409 | 413 | 415 | 422, _) => AccountError::Invalid,
        (503, _) => AccountError::Unavailable,
        _ => AccountError::Server,
    }
}

fn retry_after(headers: &HeaderMap) -> u32 {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(60)
        .clamp(1, MAX_RETRY_AFTER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_are_https_or_loopback_http_with_no_extras() {
        assert_eq!(
            Origin::parse("https://api.zephium.app").unwrap().as_str(),
            "https://api.zephium.app"
        );
        assert!(Origin::parse("http://127.0.0.1:8787").is_ok());
        assert!(Origin::parse("http://localhost:8787/").is_ok());
        for refused in [
            "http://api.zephium.app",
            "https://api.zephium.app/v1",
            "https://user@api.zephium.app",
            "https://api.zephium.app/?x=1",
            "ftp://api.zephium.app",
            "not a url",
        ] {
            assert_eq!(
                Origin::parse(refused),
                Err(AccountError::Invalid),
                "{refused}"
            );
        }
    }

    #[test]
    fn billing_links_must_open_on_the_billing_provider() {
        assert!(billing_link("https://polar.sh/checkout/abc").is_ok());
        assert!(billing_link("https://sandbox.polar.sh/portal").is_ok());
        assert!(billing_link("https://polar.sh.evil.example/x").is_err());
        assert!(billing_link("http://polar.sh/x").is_err());
    }

    #[test]
    fn errors_are_closed_and_honour_retry_after() {
        let mut headers = HeaderMap::new();
        let body = br#"{"error":{"code":"invalid_code"}}"#;
        assert_eq!(
            classify(StatusCode::UNAUTHORIZED, &headers, body),
            AccountError::InvalidCode
        );
        assert_eq!(
            classify(StatusCode::UNAUTHORIZED, &headers, b"{}"),
            AccountError::SignedOut
        );
        headers.insert(RETRY_AFTER, HeaderValue::from_static("42"));
        assert_eq!(
            classify(StatusCode::TOO_MANY_REQUESTS, &headers, b""),
            AccountError::RateLimited { retry_after: 42 }
        );
        assert_eq!(
            classify(StatusCode::BAD_GATEWAY, &HeaderMap::new(), b"<html>"),
            AccountError::Server
        );
    }
}
