use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::vault::{AccountVault, VaultFault};
use crate::wire::Purchase;
use crate::{AccountError, AccountSettings, AccountToken, Service};

#[derive(Default)]
struct MemorySettings(Mutex<HashMap<String, String>>);

impl AccountSettings for MemorySettings {
    fn get(&self, key: &str) -> Option<String> {
        self.0.lock().unwrap().get(key).cloned()
    }
    fn set(&self, key: String, value: String) -> bool {
        let mut map = self.0.lock().unwrap();
        if value.is_empty() {
            map.remove(&key);
        } else {
            map.insert(key, value);
        }
        true
    }
}

#[derive(Default)]
struct MemoryVault(Mutex<Option<String>>);

impl AccountVault for MemoryVault {
    fn load(&self) -> Result<AccountToken, VaultFault> {
        let token = self.0.lock().unwrap().clone().ok_or(VaultFault::Missing)?;
        AccountToken::new(token).map_err(|_| VaultFault::Inaccessible)
    }
    fn store(&self, token: &AccountToken) -> Result<(), VaultFault> {
        *self.0.lock().unwrap() = Some(token.expose().to_owned());
        Ok(())
    }
    fn clear(&self) -> Result<(), VaultFault> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
    fn present(&self) -> Result<bool, VaultFault> {
        Ok(self.0.lock().unwrap().is_some())
    }
}

#[derive(Clone, Debug)]
struct Seen {
    line: String,
    headers: String,
    body: String,
}

type Route = Box<dyn Fn(&Seen) -> (u16, String, String) + Send + Sync>;

/// A loopback account server answering each request with `route`.
async fn server(route: Route) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let route = Arc::new(route);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let route = route.clone();
            let log = log.clone();
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let mut chunk = [0; 4096];
                let (head, body) = loop {
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    buffer.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buffer).to_string();
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let length = head
                            .lines()
                            .find_map(|line| {
                                line.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if body.len() >= length {
                            break (head.to_owned(), body.to_owned());
                        }
                    }
                };
                let (line, headers) = head.split_once("\r\n").unwrap_or((&head, ""));
                let seen = Seen {
                    line: line.to_owned(),
                    headers: headers.to_ascii_lowercase(),
                    body,
                };
                log.lock().unwrap().push(seen.clone());
                let (status, extra, body) = route(&seen);
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n{extra}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes()).await;
            });
        }
    });
    (origin, seen)
}

fn account_routes(seen: &Seen) -> (u16, String, String) {
    let line = seen.line.as_str();
    let signed = seen.headers.contains("authorization: bearer token-1");
    match line.split(' ').take(2).collect::<Vec<_>>()[..] {
        ["POST", "/api/auth/email-otp/send-verification-otp"] => (202, "".into(), r#"{"accepted":true}"#.into()),
        ["POST", "/api/auth/sign-in/email-otp"] if seen.body.contains("\"123456\"") => (
            200,
            "set-auth-token: token-1\r\n".into(),
            r#"{"user":{}}"#.into(),
        ),
        ["POST", "/api/auth/sign-in/email-otp"] => (401, "".into(), r#"{"error":{"code":"invalid_code"}}"#.into()),
        [_, path] if path.starts_with("/v1/") && !signed => (401, "".into(), r#"{"error":{"code":"unauthorized"}}"#.into()),
        ["GET", "/v1/account"] => (200, "".into(), r#"{"id":"u1","email":"a@b.co","emailVerified":true}"#.into()),
        ["GET", "/v1/billing"] => (200, "".into(), r#"{"subscription":{"status":"inactive"},"paid":false,"offers":{"subscriptions":[{"plan":"pro","interval":"monthly"}],"topups":[]}}"#.into()),
        ["GET", "/v1/usage"] => (200, "".into(), r#"{"plan":"free","allowance":{"total_units":"100","used_units":"25","reserved_units":"0"},"purchased_remaining":"0"}"#.into()),
        ["POST", "/v1/billing/checkout"] => (200, "".into(), r#"{"url":"https://sandbox.polar.sh/checkout/x"}"#.into()),
        ["POST", "/v1/logout"] => (204, "".into(), String::new()),
        _ => (404, "".into(), r#"{"error":{"code":"not_found"}}"#.into()),
    }
}

fn service(origin: &str, vault: Arc<MemoryVault>, settings: Arc<MemorySettings>) -> Service {
    settings.set("account.server".into(), origin.into());
    Service::new(settings, vault, "Zephium 1.0 · macOS").unwrap()
}

#[tokio::test]
async fn email_code_signs_in_stores_the_bearer_and_loads_the_plan() {
    let (origin, seen) = server(Box::new(account_routes)).await;
    let vault = Arc::new(MemoryVault::default());
    let settings = Arc::new(MemorySettings::default());
    let account = service(&origin, vault.clone(), settings.clone());

    assert!(!account.signed_in());
    account.send_code("  a@b.co ").await.unwrap();
    assert_eq!(account.view().pending_email.as_deref(), Some("a@b.co"));
    assert!(account.view().resend_in.is_some());
    assert!(matches!(
        account.send_code("a@b.co").await,
        Err(AccountError::RateLimited { .. })
    ));

    assert_eq!(
        account.verify("12345").await,
        Err(AccountError::InvalidCode)
    );
    assert_eq!(
        account.verify("000000").await,
        Err(AccountError::InvalidCode)
    );
    account.verify("123456").await.unwrap();

    let view = account.view();
    assert!(view.signed_in);
    assert_eq!(view.pending_email, None);
    assert_eq!(view.profile.unwrap().email, "a@b.co");
    assert_eq!(view.usage.as_ref().unwrap().used_percent(), Some(25));
    assert_eq!(account.plan().as_deref(), Some("free"));
    assert_eq!(vault.load().unwrap().expose(), "token-1");

    let requests = seen.lock().unwrap().clone();
    let sign_in = requests
        .iter()
        .find(|r| {
            r.line.starts_with("POST /api/auth/sign-in/email-otp") && r.body.contains("123456")
        })
        .unwrap();
    assert!(sign_in
        .headers
        .contains("x-zephium-device: zephium 1.0 · macos"));
    assert!(!sign_in.headers.contains("authorization"));

    // A restart finds the session from the vault and the cached account.
    let again = Service::new(settings.clone(), vault.clone(), "x").unwrap();
    assert!(again.signed_in());
    assert_eq!(again.view().profile.unwrap().email, "a@b.co");
}

#[tokio::test]
async fn a_refused_bearer_ends_the_session_everywhere() {
    let (origin, _) = server(Box::new(account_routes)).await;
    let vault = Arc::new(MemoryVault::default());
    vault
        .store(&AccountToken::new("revoked".into()).unwrap())
        .unwrap();
    let settings = Arc::new(MemorySettings::default());
    let account = service(&origin, vault.clone(), settings.clone());
    assert!(account.signed_in());

    assert_eq!(account.refresh(true).await, Err(AccountError::SignedOut));
    assert!(!account.signed_in());
    assert_eq!(vault.present(), Ok(false));
    assert_eq!(settings.get("account.cache"), None);
    assert_eq!(account.bearer().err(), Some(AccountError::SignedOut));
}

#[tokio::test]
async fn checkout_opens_only_on_the_billing_provider_and_is_watched() {
    let (origin, seen) = server(Box::new(account_routes)).await;
    let vault = Arc::new(MemoryVault::default());
    vault
        .store(&AccountToken::new("token-1".into()).unwrap())
        .unwrap();
    let account = service(&origin, vault, Arc::new(MemorySettings::default()));

    let url = account
        .checkout(&Purchase::Plan {
            plan: "pro".into(),
            interval: "monthly".into(),
        })
        .await
        .unwrap();
    assert_eq!(url.host_str(), Some("sandbox.polar.sh"));
    assert!(account.checkout_open());
    let body = seen
        .lock()
        .unwrap()
        .iter()
        .find(|r| r.line.starts_with("POST /v1/billing/checkout"))
        .unwrap()
        .body
        .clone();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"plan": "pro", "interval": "monthly"})
    );

    // Without the on-demand refresh route, billing falls back to a plain read.
    let billing = account.refresh_billing().await.unwrap();
    assert!(!billing.paid);
}

#[tokio::test]
async fn sign_out_clears_locally_even_when_the_server_is_gone() {
    let vault = Arc::new(MemoryVault::default());
    vault
        .store(&AccountToken::new("token-1".into()).unwrap())
        .unwrap();
    let settings = Arc::new(MemorySettings::default());
    let account = service("http://127.0.0.1:9", vault.clone(), settings);
    account.sign_out().await.unwrap();
    assert!(!account.signed_in());
    assert_eq!(vault.present(), Ok(false));
}

#[tokio::test]
async fn changing_server_ends_the_session_and_remembers_only_custom_servers() {
    let vault = Arc::new(MemoryVault::default());
    vault
        .store(&AccountToken::new("token-1".into()).unwrap())
        .unwrap();
    let settings = Arc::new(MemorySettings::default());
    let account = service("http://127.0.0.1:9", vault.clone(), settings.clone());

    assert_eq!(
        account.set_server(Some("http://example.com")).await,
        Err(AccountError::Invalid)
    );
    assert!(account.signed_in());

    account.set_server(None).await.unwrap();
    assert!(!account.signed_in());
    assert_eq!(account.origin(), crate::DEFAULT_ORIGIN);
    assert_eq!(settings.get("account.server"), None);
    assert!(!account.view().custom_server);
}
