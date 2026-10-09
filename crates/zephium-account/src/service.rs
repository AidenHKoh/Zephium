//! The account as the app sees it: one per install, shared by every profile.
//!
//! State changes only here. Callers read a [`View`] and are told through the
//! observer when it changes. Async results carry the epoch they started in, so
//! a reply that arrives after sign-out or a server change is dropped.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;
use url::Url;

use crate::client::{Client, ConfigReply, Origin};
use crate::vault::{AccountVault, VaultFault};
use crate::wire::{
    Billing, ClientConfig, Profile, Purchase, Receipt, Session, Usage, UsageHistory,
};
use crate::{older_than, AccountError, AccountToken, DEFAULT_ORIGIN};

const SERVER_KEY: &str = "account.server";
const CACHE_KEY: &str = "account.cache";
const MAX_CACHE_BYTES: usize = 64 * 1024;
const MAX_EMAIL_BYTES: usize = 254;
const MAX_NAME_CHARS: usize = 100;
/// Matches the service's resend cooldown; the server's own answer still wins.
const RESEND_AFTER: Duration = Duration::from_secs(60);
/// Plan and usage older than this are fetched again when someone looks.
const STALE_AFTER: Duration = Duration::from_secs(60);
const CHECKOUT_WATCH: Duration = Duration::from_secs(15 * 60);
const MAX_DEVICE_CHARS: usize = 64;
/// The service allows six billing refreshes a minute per account.
const BILLING_REFRESHES: usize = 6;
const BILLING_WINDOW: Duration = Duration::from_secs(60);
const MAX_UNSETTLED: usize = 8;

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
    /// The server asks for a newer Zephium than this one.
    pub update_required: bool,
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
    /// Hosted calls whose charge was not confirmed when they ended, as last
    /// read from their receipts.
    pub unsettled: Vec<Receipt>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Cache {
    #[serde(default)]
    origin: String,
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
    config_etag: Option<String>,
    config_until: Option<Instant>,
    profile: Option<Profile>,
    billing: Option<Billing>,
    usage: Option<Usage>,
    refreshed: Option<Instant>,
    checkout_until: Option<Instant>,
    billing_refreshes: VecDeque<Instant>,
    unsettled: Vec<Receipt>,
}

impl Inner {
    fn origin(&self) -> String {
        self.client.origin().as_str().to_owned()
    }

    fn clear_account(&mut self) {
        self.token = None;
        self.profile = None;
        self.billing = None;
        self.usage = None;
        self.refreshed = None;
        self.checkout_until = None;
        self.billing_refreshes.clear();
        self.unsettled.clear();
    }
}

type Observer = Box<dyn Fn() + Send + Sync>;

pub struct Service {
    settings: Arc<dyn AccountSettings>,
    vault: Arc<dyn AccountVault>,
    device: String,
    version: String,
    inner: Mutex<Inner>,
    observer: OnceLock<Observer>,
}

impl Service {
    /// Reads the configured server and the last known account from settings.
    /// Signed in means a bearer for that server is in the vault; it is not
    /// read until needed. `version` is this build's, for the server's minimum.
    pub fn new(
        settings: Arc<dyn AccountSettings>,
        vault: Arc<dyn AccountVault>,
        device: &str,
        version: &str,
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
        let signed_in = matches!(vault.present(origin.as_str()), Ok(true));
        let cache: Cache = settings
            .get(CACHE_KEY)
            .filter(|_| signed_in)
            .and_then(|text| serde_json::from_str::<Cache>(&text).ok())
            .filter(|cache| cache.origin == origin.as_str())
            .unwrap_or_default();
        let device: String = device
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_DEVICE_CHARS)
            .collect();
        Ok(Self {
            settings,
            vault,
            device,
            version: version.to_owned(),
            inner: Mutex::new(Inner {
                client: Arc::new(Client::new(origin)?),
                custom_server,
                epoch: 0,
                signed_in,
                token: None,
                pending: None,
                config: None,
                config_etag: None,
                config_until: None,
                profile: cache.profile,
                billing: cache.billing,
                usage: cache.usage,
                refreshed: None,
                checkout_until: None,
                billing_refreshes: VecDeque::new(),
                unsettled: Vec::new(),
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
            update_required: inner
                .config
                .as_ref()
                .and_then(|config| config.min_client_version.as_deref())
                .is_some_and(|minimum| older_than(&self.version, minimum)),
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
            unsettled: inner.unsettled.clone(),
        }
    }

    pub fn signed_in(&self) -> bool {
        self.lock().signed_in
    }

    pub fn origin(&self) -> String {
        self.lock().origin()
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

    /// What the server offers, read before sign-in. A copy younger than the
    /// server's max-age (five minutes at most) is reused; an older one is
    /// revalidated by its ETag. A failure leaves the last answer.
    pub async fn load_config(&self) -> Result<ClientConfig, AccountError> {
        let (client, epoch, etag) = {
            let inner = self.lock();
            if let (Some(config), Some(until)) = (&inner.config, inner.config_until) {
                if Instant::now() < until {
                    return Ok(config.clone());
                }
            }
            let etag = inner.config.as_ref().and(inner.config_etag.clone());
            (inner.client.clone(), inner.epoch, etag)
        };
        let reply = client.client_config(etag.as_deref()).await?;
        let config = {
            let mut inner = self.lock();
            if inner.epoch != epoch {
                return Err(AccountError::Invalid);
            }
            match reply {
                ConfigReply::Fresh {
                    config,
                    etag,
                    max_age,
                } => {
                    inner.config = Some(config.clone());
                    inner.config_etag = etag;
                    inner.config_until = Some(Instant::now() + max_age);
                    config
                }
                ConfigReply::Unchanged { max_age } => {
                    inner.config_until = Some(Instant::now() + max_age);
                    inner.config.clone().ok_or(AccountError::Protocol)?
                }
            }
        };
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

    /// Sends a sign-in code to the signed-in address, for a change that needs
    /// a fresh session.
    pub async fn send_reauth_code(&self) -> Result<(), AccountError> {
        let email = {
            let inner = self.lock();
            if !inner.signed_in {
                return Err(AccountError::SignedOut);
            }
            inner.profile.as_ref().map(|profile| profile.email.clone())
        }
        .ok_or(AccountError::Invalid)?;
        self.send_code(&email).await
    }

    /// Forgets the address a code was sent to.
    pub fn cancel_code(&self) {
        self.lock().pending = None;
        self.notify();
    }

    /// Signs in with the code sent to the pending address. A session this
    /// install already had on the server is replaced and revoked.
    pub async fn verify(&self, code: &str) -> Result<(), AccountError> {
        let code = code.trim();
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AccountError::InvalidCode);
        }
        let previous = self.bearer().ok();
        let (client, epoch, email, origin) = {
            let inner = self.lock();
            let email = inner
                .pending
                .as_ref()
                .map(|p| p.email.clone())
                .ok_or(AccountError::Invalid)?;
            (inner.client.clone(), inner.epoch, email, inner.origin())
        };
        let token = client.verify_code(&email, code, &self.device).await?;
        if self.lock().epoch != epoch {
            return Err(AccountError::Invalid);
        }
        self.vault
            .store(&origin, &token)
            .map_err(|_| AccountError::Vault)?;
        let token = Arc::new(token);
        {
            let mut inner = self.lock();
            if inner.epoch != epoch {
                drop(inner);
                let _ = self.vault.clear(&origin);
                return Err(AccountError::Invalid);
            }
            inner.epoch += 1;
            inner.clear_account();
            inner.signed_in = true;
            inner.token = Some(token);
            inner.pending = None;
        }
        self.notify();
        if let Some(previous) = previous {
            let _ = client.sign_out(&previous).await;
        }
        let _ = self.refresh(true).await;
        Ok(())
    }

    /// The bearer for this session, read from the vault once.
    pub fn bearer(&self) -> Result<Arc<AccountToken>, AccountError> {
        let (epoch, origin) = {
            let inner = self.lock();
            if !inner.signed_in {
                return Err(AccountError::SignedOut);
            }
            if let Some(token) = &inner.token {
                return Ok(token.clone());
            }
            (inner.epoch, inner.origin())
        };
        let token = match self.vault.load(&origin) {
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

    /// A hosted call ran: what was used changed, so the next look asks again.
    pub fn usage_changed(&self) {
        self.lock().refreshed = None;
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

    /// Changes the name shown for the account; `None` or blank clears it.
    pub async fn set_name(&self, name: Option<&str>) -> Result<(), AccountError> {
        let name = name.map(str::trim).filter(|name| !name.is_empty());
        if name.is_some_and(|name| {
            name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control)
        }) {
            return Err(AccountError::Invalid);
        }
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        let saved = self.signed_out_on_401(epoch, client.set_name(&token, name).await)?;
        {
            let mut inner = self.lock();
            if inner.epoch != epoch {
                return Ok(());
            }
            if let Some(profile) = inner.profile.as_mut() {
                profile.name = saved;
            }
            self.persist(&inner);
        }
        self.notify();
        Ok(())
    }

    /// Sends a code to `email`, the address the account is moving to. The
    /// service asks for a session from the last ten minutes first.
    pub async fn send_email_change_code(&self, email: &str) -> Result<(), AccountError> {
        let email = email.trim();
        let current = self.lock().profile.as_ref().map(|p| p.email.clone());
        if !plausible_email(email)
            || current.is_some_and(|current| current.eq_ignore_ascii_case(email))
        {
            return Err(AccountError::Invalid);
        }
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.send_email_change_code(&token, email).await)
    }

    /// Moves the account to `email`. The service ends every session when it
    /// does, so this install signs in again afterwards.
    pub async fn change_email(&self, email: &str, code: &str) -> Result<(), AccountError> {
        let email = email.trim();
        let code = code.trim();
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AccountError::InvalidCode);
        }
        if !plausible_email(email) {
            return Err(AccountError::Invalid);
        }
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.change_email(&token, email, code).await)?;
        self.forget(epoch);
        Ok(())
    }

    /// The account's export as the server states it; no credentials in it.
    pub async fn export(&self) -> Result<Value, AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.export(&token).await)
    }

    /// One page of hosted request history.
    pub async fn usage_history(&self, cursor: Option<&str>) -> Result<UsageHistory, AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        self.signed_out_on_401(epoch, client.usage_history(&token, cursor).await)
    }

    /// One hosted request's accounting.
    pub async fn receipt(&self, id: &str) -> Result<Receipt, AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        let receipt = self.signed_out_on_401(epoch, client.receipt(&token, id).await)?;
        let changed = {
            let mut inner = self.lock();
            let known =
                inner.epoch == epoch && inner.unsettled.iter().any(|held| held.id == receipt.id);
            if known {
                for held in &mut inner.unsettled {
                    if held.id == receipt.id {
                        *held = receipt.clone();
                    }
                }
            }
            known
        };
        if changed {
            self.notify();
        }
        Ok(receipt)
    }

    /// Reads the receipt of a hosted call whose charge was not confirmed when
    /// it ended, and keeps it for the account page until it settles.
    pub async fn confirm_request(&self, id: &str) -> Result<Receipt, AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        let receipt = self.signed_out_on_401(epoch, client.receipt(&token, id).await)?;
        {
            let mut inner = self.lock();
            if inner.epoch != epoch {
                return Ok(receipt);
            }
            inner.unsettled.retain(|held| held.id != receipt.id);
            inner.unsettled.insert(0, receipt.clone());
            inner.unsettled.truncate(MAX_UNSETTLED);
            inner.refreshed = None;
        }
        self.notify();
        Ok(receipt)
    }

    /// Asks the server to check billing now, after a checkout or portal visit.
    /// Held to the service's limit of six a minute.
    pub async fn refresh_billing(&self) -> Result<Billing, AccountError> {
        let token = self.bearer()?;
        {
            let mut inner = self.lock();
            let now = Instant::now();
            while inner
                .billing_refreshes
                .front()
                .is_some_and(|at| now.duration_since(*at) >= BILLING_WINDOW)
            {
                inner.billing_refreshes.pop_front();
            }
            if inner.billing_refreshes.len() >= BILLING_REFRESHES {
                let oldest = inner.billing_refreshes.front().copied().unwrap_or(now);
                let wait = BILLING_WINDOW.saturating_sub(now.duration_since(oldest));
                return Err(AccountError::RateLimited {
                    retry_after: wait.as_secs().max(1) as u32,
                });
            }
            inner.billing_refreshes.push_back(now);
        }
        let (client, epoch) = self.client();
        let billing = match client.refresh_billing(&token).await {
            Err(AccountError::Invalid | AccountError::NotFound) => client.billing(&token).await,
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
        let origin = self.origin();
        if let Ok(token) = self.bearer() {
            let _ = client.sign_out(&token).await;
        }
        self.forget(epoch);
        if self.vault.clear(&origin).is_err() {
            return Err(AccountError::Vault);
        }
        Ok(())
    }

    /// Ends every session of the account, this one included. Unlike signing
    /// out here, nothing changes unless the server confirms.
    pub async fn sign_out_everywhere(&self) -> Result<(), AccountError> {
        let token = self.bearer()?;
        let (client, epoch) = self.client();
        match client.sign_out_everywhere(&token).await {
            Ok(()) | Err(AccountError::SignedOut) => {
                self.forget(epoch);
                Ok(())
            }
            Err(error) => Err(error),
        }
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
        Ok(())
    }

    /// Points the app at another account server, or back at Zephium's with
    /// `None`. Each server keeps its own session in the vault, so switching
    /// back finds it again; nothing is sent to either server.
    pub async fn set_server(&self, server: Option<&str>) -> Result<(), AccountError> {
        let origin = match server.map(str::trim).filter(|s| !s.is_empty()) {
            Some(text) => Origin::parse(text)?,
            None => Origin::parse(DEFAULT_ORIGIN)?,
        };
        let custom = origin.as_str() != DEFAULT_ORIGIN;
        let signed_in = matches!(self.vault.present(origin.as_str()), Ok(true));
        let client = Arc::new(Client::new(origin.clone())?);
        let stored = if custom {
            origin.as_str().to_owned()
        } else {
            String::new()
        };
        if !self.settings.set(SERVER_KEY.to_owned(), stored) {
            return Err(AccountError::Unavailable);
        }
        self.settings.set(CACHE_KEY.to_owned(), String::new());
        {
            let mut inner = self.lock();
            inner.epoch += 1;
            inner.client = client;
            inner.custom_server = custom;
            inner.config = None;
            inner.config_etag = None;
            inner.config_until = None;
            inner.pending = None;
            inner.clear_account();
            inner.signed_in = signed_in;
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

    /// Drops the session from memory and settings. The vault entry for this
    /// server is cleared too, since a refused bearer will never work again;
    /// other servers' entries are left alone.
    fn forget(&self, epoch: u64) {
        let origin = {
            let mut inner = self.lock();
            if inner.epoch != epoch || !inner.signed_in {
                return;
            }
            inner.epoch += 1;
            inner.signed_in = false;
            inner.clear_account();
            inner.origin()
        };
        let _ = self.vault.clear(&origin);
        self.settings.set(CACHE_KEY.to_owned(), String::new());
        self.notify();
    }

    fn persist(&self, inner: &Inner) {
        let cache = Cache {
            origin: inner.origin(),
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
