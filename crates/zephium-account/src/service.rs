//! The account as the app sees it: one per install, shared by every profile.
//!
//! State changes only here. Callers read a [`View`] and are told through the
//! observer when it changes. Async results carry the epoch they started in, so
//! a reply that arrives after sign-out or a server change is dropped.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use url::Url;

use crate::client::{Client, Origin};
use crate::vault::{AccountVault, VaultFault};
use crate::wire::{Billing, ClientConfig, Profile, Purchase, Session, Usage};
use crate::{AccountError, AccountToken, DEFAULT_ORIGIN};

const SERVER_KEY: &str = "account.server";
const CACHE_KEY: &str = "account.cache";
const MAX_CACHE_BYTES: usize = 64 * 1024;
const MAX_EMAIL_BYTES: usize = 254;
/// Matches the service's resend cooldown; the server's own answer still wins.
const RESEND_AFTER: Duration = Duration::from_secs(60);
/// Plan and usage older than this are fetched again when someone looks.
const STALE_AFTER: Duration = Duration::from_secs(60);
const CHECKOUT_WATCH: Duration = Duration::from_secs(15 * 60);
const MAX_DEVICE_BYTES: usize = 64;

/// App settings, installed by the composition root.
pub trait AccountSettings: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    /// Stores a value; an empty value clears it.
    fn set(&self, key: String, value: String) -> bool;
}

/// What the person sees. Never the bearer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub server: String,
    pub custom_server: bool,
    pub config: Option<ClientConfig>,
    pub signed_in: bool,
    /// The address a code was sent to, while waiting for it.
    pub pending_email: Option<String>,
    /// Seconds until another code may be sent.
    pub resend_in: Option<u32>,
    pub profile: Option<Profile>,
    pub billing: Option<Billing>,
    pub usage: Option<Usage>,
    /// A checkout was opened and has not been seen to complete.
    pub checkout_open: bool,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Cache {
    profile: Option<Profile>,
    billing: Option<Billing>,
    usage: Option<Usage>,
}

struct Pending {
    email: String,
    resend_at: Instant,
}

struct Inner {
    client: Arc<Client>,
    custom_server: bool,
    epoch: u64,
    signed_in: bool,
    token: Option<Arc<AccountToken>>,
    pending: Option<Pending>,
    config: Option<ClientConfig>,
    profile: Option<Profile>,
    billing: Option<Billing>,
    usage: Option<Usage>,
    refreshed: Option<Instant>,
    checkout_until: Option<Instant>,
}

type Observer = Box<dyn Fn() + Send + Sync>;

pub struct Service {
    settings: Arc<dyn AccountSettings>,
    vault: Arc<dyn AccountVault>,
    device: String,
    inner: Mutex<Inner>,
    observer: OnceLock<Observer>,
}

impl Service {
    /// Reads the configured server and the last known account from settings.
    /// Signed in means a bearer is in the vault; it is not read until needed.
    pub fn new(
        settings: Arc<dyn AccountSettings>,
        vault: Arc<dyn AccountVault>,
        device: &str,
    ) -> Result<Self, AccountError> {
        let custom = settings
            .get(SERVER_KEY)
            .filter(|value| !value.is_empty())
            .and_then(|value| Origin::parse(&value).ok());
        let custom_server = custom.is_some();
        let origin = match custom {
            Some(origin) => origin,
            None => Origin::parse(DEFAULT_ORIGIN)?,
        };
        let signed_in = matches!(vault.present(), Ok(true));
        let cache: Cache = settings
            .get(CACHE_KEY)
            .filter(|_| signed_in)
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let device: String = device
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_DEVICE_BYTES)
            .collect();
        Ok(Self {
            settings,
            vault,
            device,
            inner: Mutex::new(Inner {
                client: Arc::new(Client::new(origin)?),
                custom_server,
                epoch: 0,
                signed_in,
                token: None,
                pending: None,
                config: None,
                profile: cache.profile,
                billing: cache.billing,
                usage: cache.usage,
                refreshed: None,
                checkout_until: None,
            }),
            observer: OnceLock::new(),
        })
    }

    /// Called after every change a view would show.
    pub fn set_observer(&self, observer: Observer) {
        let _ = self.observer.set(observer);
    }

    fn notify(&self) {
        if let Some(observer) = self.observer.get() {
            observer();
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn view(&self) -> View {
        let inner = self.lock();
        let now = Instant::now();
        View {
            server: inner.client.origin().as_str().to_owned(),
            custom_server: inner.custom_server,
            config: inner.config.clone(),
            signed_in: inner.signed_in,
            pending_email: inner.pending.as_ref().map(|p| p.email.clone()),
            resend_in: inner
                .pending
                .as_ref()
                .map(|p| p.resend_at.saturating_duration_since(now).as_secs() as u32)
                .filter(|secs| *secs > 0),
            profile: inner.profile.clone(),
            billing: inner.billing.clone(),
            usage: inner.usage.clone(),
            checkout_open: inner.checkout_until.is_some_and(|until| until > now),
        }
    }

    pub fn signed_in(&self) -> bool {
        self.lock().signed_in
    }

    pub fn origin(&self) -> String {
        self.lock().client.origin().as_str().to_owned()
    }

    /// The plan the server reports for the signed-in account.
    pub fn plan(&self) -> Option<String> {
        let inner = self.lock();
        if !inner.signed_in {
            return None;
        }
        inner.usage.as_ref().and_then(|usage| usage.plan.clone())
    }

    fn client(&self) -> (Arc<Client>, u64) {
        let inner = self.lock();
        (inner.client.clone(), inner.epoch)
    }

    /// What the server offers, read before sign-in; a failure leaves the last answer.
    pub async fn load_config(&self) -> Result<ClientConfig, AccountError> {
        let (client, epoch) = self.client();
        let config = client.client_config().await?;
        {
            let mut inner = self.lock();
            if inner.epoch == epoch {
                inner.config = Some(config.clone());
            }
        }
        self.notify();
        Ok(config)
    }

    /// Sends a sign-in code. The first code an address receives creates its account.
    pub async fn send_code(&self, email: &str) -> Result<(), AccountError> {
        let email = email.trim().to_owned();
        if !plausible_email(&email) {
            return Err(AccountError::Invalid);
        }
        {
            let inner = self.lock();
            if let Some(pending) = &inner.pending {
                let wait = pending.resend_at.saturating_duration_since(Instant::now());
                if pending.email == email && !wait.is_zero() {
                    return Err(AccountError::RateLimited {
                        retry_after: wait.as_secs().max(1) as u32,
                    });
                }
            }
        }
        let (client, epoch) = self.client();
        let sent = client.send_code(&email).await;
        let resend_at = match sent {
            Ok(()) => Instant::now() + RESEND_AFTER,
            Err(AccountError::RateLimited { retry_after }) => {
                Instant::now() + Duration::from_secs(retry_after.into())
            }
            Err(error) => return Err(error),
        };
        {
            let mut inner = self.lock();
            if inner.epoch == epoch {
                inner.pending = Some(Pending { email, resend_at });
            }
        }
        self.notify();
        sent
    }

    /// Forgets the address a code was sent to.
    pub fn cancel_code(&self) {
        self.lock().pending = None;
        self.notify();
    }

    /// Signs in with the code sent to the pending address.
    pub async fn verify(&self, code: &str) -> Result<(), AccountError> {
        let code = code.trim();
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AccountError::InvalidCode);
        }
        let (client, epoch, email) = {
            let inner = self.lock();
            let email = inner
                .pending
                .as_ref()
                .map(|p| p.email.clone())
                .ok_or(AccountError::Invalid)?;
            (inner.client.clone(), inner.epoch, email)
        };
        let token = client.verify_code(&email, code, &self.device).await?;
        if self.lock().epoch != epoch {
            return Err(AccountError::Invalid);
        }
        self.vault.store(&token).map_err(|_| AccountError::Vault)?;
        let token = Arc::new(token);
        {
            let mut inner = self.lock();
            if inner.epoch != epoch {
                drop(inner);
                let _ = self.vault.clear();
                return Err(AccountError::Invalid);
            }
            inner.epoch += 1;
            inner.signed_in = true;
            inner.token = Some(token);
            inner.pending = None;
            inner.profile = None;
            inner.billing = None;
            inner.usage = None;
            inner.refreshed = None;
        }
        self.notify();
        let _ = self.refresh(true).await;
        Ok(())
    }

    /// The bearer for this session, read from the vault once.
    pub fn bearer(&self) -> Result<Arc<AccountToken>, AccountError> {
        let epoch = {
            let inner = self.lock();
            if !inner.signed_in {
                return Err(AccountError::SignedOut);
            }
            if let Some(token) = &inner.token {
                return Ok(token.clone());
            }
            inner.epoch
        };
        let loaded = self.vault.load();
        let token = match loaded {
            Ok(token) => Arc::new(token),
            Err(VaultFault::Missing) => {
                self.forget(epoch);
                return Err(AccountError::SignedOut);
            }
            Err(VaultFault::Inaccessible) => return Err(AccountError::Vault),
        };
        let mut inner = self.lock();
        if inner.epoch != epoch || !inner.signed_in {
            return Err(AccountError::SignedOut);
        }
        Ok(inner.token.get_or_insert(token).clone())
    }

    /// The hosted gateway refused the bearer: the session is over.
    pub fn bearer_refused(&self) {
        let epoch = self.lock().epoch;
        self.forget(epoch);
    }

    /// Fetches profile, plan and usage; skipped while fresh unless forced.
    pub async fn refresh(&self, force: bool) -> Result<(), AccountError> {
        {
            let inner = self.lock();
            if !inner.signed_in {
                return Ok(());
            }
            if !force && inner.refreshed.is_some_and(|at| at.elapsed() < STALE_AFTER) {
                return Ok(());
            }
        }
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        let (profile, billing, usage) = tokio::join!(
            client.profile(&token),
            client.billing(&token),
            client.usage(&token),
        );
        if [
            profile.as_ref().err(),
            billing.as_ref().err(),
            usage.as_ref().err(),
        ]
        .contains(&Some(&AccountError::SignedOut))
        {
            self.forget(epoch);
            return Err(AccountError::SignedOut);
        }
        let profile = profile?;
        {
            let mut inner = self.lock();
            if inner.epoch != epoch {
                return Ok(());
            }
            inner.profile = Some(profile);
            // A server with billing or hosted models turned off answers those
            // with an error; the account itself still works.
            inner.billing = billing.ok().or(inner.billing.take());
            inner.usage = usage.ok().or(inner.usage.take());
            inner.refreshed = Some(Instant::now());
            self.persist(&inner);
        }
        self.notify();
        Ok(())
    }

    /// Asks the server to check billing now, after a checkout or portal visit.
    pub async fn refresh_billing(&self) -> Result<Billing, AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        let billing = match client.refresh_billing(&token).await {
            Err(AccountError::Invalid) => client.billing(&token).await,
            other => other,
        };
        let billing = match billing {
            Err(AccountError::SignedOut) => {
                self.forget(epoch);
                return Err(AccountError::SignedOut);
            }
            other => other?,
        };
        let usage = client.usage(&token).await.ok();
        {
            let mut inner = self.lock();
            if inner.epoch == epoch {
                let before = inner.billing.clone();
                inner.billing = Some(billing.clone());
                if usage.is_some() {
                    inner.usage = usage;
                }
                if before.is_some_and(|before| before != billing) {
                    inner.checkout_until = None;
                }
                self.persist(&inner);
            }
        }
        self.notify();
        Ok(billing)
    }

    /// A checkout link on the billing provider, to open in a browser tab.
    pub async fn checkout(&self, purchase: &Purchase) -> Result<Url, AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        let url = self.signed_out_on_401(epoch, client.checkout(&token, purchase).await)?;
        self.lock().checkout_until = Some(Instant::now() + CHECKOUT_WATCH);
        self.notify();
        Ok(url)
    }

    /// The billing provider's portal for managing the subscription.
    pub async fn portal(&self) -> Result<Url, AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.portal(&token).await)
    }

    /// Whether an opened checkout is still being watched for.
    pub fn checkout_open(&self) -> bool {
        self.lock()
            .checkout_until
            .is_some_and(|until| until > Instant::now())
    }

    pub async fn sessions(&self) -> Result<Vec<Session>, AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.sessions(&token).await)
    }

    pub async fn revoke_session(&self, id: &str) -> Result<(), AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.revoke_session(&token, id).await)
    }

    /// Signs out here. The server is told when it can be reached; the vault
    /// entry goes either way.
    pub async fn sign_out(&self) -> Result<(), AccountError> {
        let (client, epoch) = self.client();
        if let Ok(token) = self.bearer() {
            let _ = client.sign_out(&token).await;
        }
        self.forget(epoch);
        let cleared = self.vault.clear();
        if cleared.is_err() {
            return Err(AccountError::Vault);
        }
        Ok(())
    }

    pub async fn send_deletion_code(&self) -> Result<(), AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.send_deletion_code(&token).await)
    }

    /// Deletes the account on the server, then everything about it here.
    pub async fn delete_account(&self, code: &str) -> Result<(), AccountError> {
        let code = code.trim();
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AccountError::InvalidCode);
        }
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.delete_account(&token, code).await)?;
        self.forget(epoch);
        let _ = self.vault.clear();
        Ok(())
    }

    /// Points the app at another account server, or back at Zephium's with
    /// `None`. The current session belongs to the old server, so it ends.
    pub async fn set_server(&self, server: Option<&str>) -> Result<(), AccountError> {
        let origin = match server.map(str::trim).filter(|s| !s.is_empty()) {
            Some(text) => Origin::parse(text)?,
            None => Origin::parse(DEFAULT_ORIGIN)?,
        };
        let custom = origin.as_str() != DEFAULT_ORIGIN;
        let client = Arc::new(Client::new(origin.clone())?);
        let _ = self.sign_out().await;
        let stored = if custom {
            origin.as_str().to_owned()
        } else {
            String::new()
        };
        if !self.settings.set(SERVER_KEY.to_owned(), stored) {
            return Err(AccountError::Unavailable);
        }
        {
            let mut inner = self.lock();
            inner.epoch += 1;
            inner.client = client;
            inner.custom_server = custom;
            inner.config = None;
            inner.pending = None;
        }
        self.notify();
        Ok(())
    }

    fn signed_out_on_401<T>(
        &self,
        epoch: u64,
        result: Result<T, AccountError>,
    ) -> Result<T, AccountError> {
        if result.as_ref().err() == Some(&AccountError::SignedOut) {
            self.forget(epoch);
        }
        result
    }

    /// Drops the session from memory and settings. The vault entry is cleared
    /// too, since a refused bearer will never work again.
    fn forget(&self, epoch: u64) {
        {
            let mut inner = self.lock();
            if inner.epoch != epoch || !inner.signed_in {
                return;
            }
            inner.epoch += 1;
            inner.signed_in = false;
            inner.token = None;
            inner.profile = None;
            inner.billing = None;
            inner.usage = None;
            inner.refreshed = None;
            inner.checkout_until = None;
        }
        let _ = self.vault.clear();
        self.settings.set(CACHE_KEY.to_owned(), String::new());
        self.notify();
    }

    fn persist(&self, inner: &Inner) {
        let cache = Cache {
            profile: inner.profile.clone(),
            billing: inner.billing.clone(),
            usage: inner.usage.clone(),
        };
        if let Ok(text) = serde_json::to_string(&cache) {
            if text.len() <= MAX_CACHE_BYTES {
                self.settings.set(CACHE_KEY.to_owned(), text);
            }
        }
    }
}

fn plausible_email(email: &str) -> bool {
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    email.len() <= MAX_EMAIL_BYTES
        && !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::plausible_email;

    #[test]
    fn emails_need_a_local_part_and_a_dotted_domain() {
        assert!(plausible_email("a@b.co"));
        assert!(plausible_email("first.last+tag@mail.example.org"));
        for refused in ["", "a", "@b.co", "a@b", "a@.co", "a@b.", "a b@c.co"] {
            assert!(!plausible_email(refused), "{refused}");
        }
    }
}
