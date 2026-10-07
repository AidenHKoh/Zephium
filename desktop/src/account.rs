//! The optional Zephium account, as the frame sees it. The bearer stays in
//! native code; the frame gets the account's state and closed faults only.
use super::*;
use zephium_account::wire::{Billing, Purchase};
use zephium_account::{AccountError, AccountSettings, Service, SystemVault, View};

const CHECKOUT_POLL: std::time::Duration = std::time::Duration::from_secs(10);
const MAX_EMAIL_INPUT_BYTES: usize = 320;
const MAX_SERVER_INPUT_BYTES: usize = 512;

static SERVICE: OnceLock<Arc<Service>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Serialize, Deserialize, specta::Type, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AccountFaultV1 {
    Offline,
    /// The session ended; sign in again.
    SignedOut,
    RateLimited,
    InvalidCode,
    SignupClosed,
    /// The service, or the part asked for, is off.
    Unavailable,
    /// Nothing has been bought, so there is nothing to manage.
    NoBillingAccount,
    Server,
    /// The Keychain or Credential Manager refused.
    Vault,
    Invalid,
}

impl From<AccountError> for AccountFaultV1 {
    fn from(error: AccountError) -> Self {
        match error {
            AccountError::Offline => Self::Offline,
            AccountError::SignedOut => Self::SignedOut,
            AccountError::RateLimited { .. } => Self::RateLimited,
            AccountError::InvalidCode => Self::InvalidCode,
            AccountError::SignupClosed => Self::SignupClosed,
            AccountError::Unavailable => Self::Unavailable,
            AccountError::NoBillingAccount => Self::NoBillingAccount,
            AccountError::Server | AccountError::Protocol => Self::Server,
            AccountError::Vault => Self::Vault,
            AccountError::Invalid => Self::Invalid,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
pub(crate) struct AccountUsageV1 {
    /// Share of the plan's allowance used, 0–100; null without an allowance.
    pub(crate) used_percent: Option<u8>,
    /// When the allowance renews, as the server states it (ISO 8601).
    pub(crate) resets_at: Option<String>,
    /// Bought usage remains beyond the plan's allowance.
    pub(crate) extra: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
pub(crate) struct AccountSubscriptionV1 {
    /// `active`, `inactive` or `unknown`.
    pub(crate) status: String,
    pub(crate) interval: Option<String>,
    pub(crate) paid_until: Option<String>,
    pub(crate) cancel_at_period_end: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
pub(crate) struct AccountPriceV1 {
    /// In the currency's minor unit, as the billing provider states it.
    pub(crate) amount_minor: u32,
    pub(crate) currency: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum AccountOfferV1 {
    Plan {
        plan: String,
        interval: String,
        name: Option<String>,
        price: Option<AccountPriceV1>,
        features: Vec<String>,
    },
    TopUp {
        id: String,
        price: Option<AccountPriceV1>,
        expires_days: Option<u32>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
pub(crate) struct AccountNoticeV1 {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) warning: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
pub(crate) struct AccountV1 {
    pub(crate) version: u16,
    /// Accounts work on this platform and build.
    pub(crate) available: bool,
    pub(crate) server: String,
    pub(crate) custom_server: bool,
    /// What the server offers; null until it has answered once.
    pub(crate) hosted_models: Option<bool>,
    pub(crate) billing: Option<bool>,
    pub(crate) signup_open: Option<bool>,
    pub(crate) signed_in: bool,
    /// The address a sign-in code was sent to, while waiting for it.
    pub(crate) pending_email: Option<String>,
    pub(crate) resend_in: Option<u32>,
    pub(crate) email: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) plan: Option<String>,
    pub(crate) usage: Option<AccountUsageV1>,
    pub(crate) subscription: Option<AccountSubscriptionV1>,
    pub(crate) offers: Vec<AccountOfferV1>,
    /// A checkout was opened and is being watched for completion.
    pub(crate) checkout_open: bool,
    pub(crate) notice: Option<AccountNoticeV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
pub(crate) struct AccountReplyV1 {
    pub(crate) account: AccountV1,
    pub(crate) fault: Option<AccountFaultV1>,
    /// Seconds to wait before trying again, with `rate_limited`.
    pub(crate) retry_after: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
pub(crate) struct AccountSessionV1 {
    pub(crate) id: String,
    pub(crate) device: Option<String>,
    pub(crate) created_at: Option<String>,
    pub(crate) last_used_at: Option<String>,
    pub(crate) current: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
pub(crate) struct AccountSessionsV1 {
    pub(crate) sessions: Vec<AccountSessionV1>,
    pub(crate) fault: Option<AccountFaultV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum AccountPurchaseV1 {
    Plan { plan: String, interval: String },
    TopUp { id: String },
}

/// The account changed; read it again.
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[tauri_specta(event_name = "zephium:account-changed")]
pub(crate) struct AccountChanged {
    pub(crate) version: u16,
}

struct AppSettings;

impl AccountSettings for AppSettings {
    fn get(&self, key: &str) -> Option<String> {
        APP_STORE.get().and_then(|store| store.app_setting(key))
    }
    fn set(&self, key: String, value: String) -> bool {
        APP_STORE
            .get()
            .is_some_and(|store| store.set_app_setting(key, value))
    }
}

/// A label the person recognises in their list of signed-in devices. It names
/// the app and the system only: no host name, model or user name.
fn device_label(app: &tauri::AppHandle) -> String {
    let system = if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    };
    format!("Zephium {} · {system}", app.package_info().version)
}

/// Creates the account service after the app store exists. A session left in
/// the vault is refreshed in the background.
pub(crate) fn install(app: &tauri::AppHandle) {
    let service = match Service::new(
        Arc::new(AppSettings),
        Arc::new(SystemVault),
        &device_label(app),
    ) {
        Ok(service) => Arc::new(service),
        Err(error) => {
            write_diagnostic(format_args!("account: install fault={error:?}"));
            return;
        }
    };
    let events = app.clone();
    service.set_observer(Box::new(move || {
        emit_to_privileged(
            &events,
            MAIN_LABEL,
            "zephium:account-changed",
            &AccountChanged { version: 1 },
        );
        #[cfg(feature = "work-product")]
        hosted::account_changed();
    }));
    if SERVICE.set(service.clone()).is_err() {
        return;
    }
    #[cfg(feature = "work-product")]
    hosted::install(service.clone());
    // Nobody who has not chosen an account, or another server, is ever
    // connected to one: the server is asked only once they have.
    if !contacted(&service) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let _ = service.load_config().await;
        if let Err(error) = service.refresh(true).await {
            write_diagnostic(format_args!("account: refresh fault={error:?}"));
        }
    });
}

/// Whether the person has chosen to involve an account server.
fn contacted(service: &Service) -> bool {
    service.signed_in() || service.view().custom_server
}

/// The signed-in account as Work's hosted model provider.
#[cfg(feature = "work-product")]
mod hosted {
    use super::*;
    use zephium_agentic::lead::{LeadCredential, LeadSecret, LeadSecretFuture};
    use zephium_app::work_models::{self, WorkCloudSession};
    use zephium_core::work::model::WorkModelError;

    static SIGNED_IN: AtomicBool = AtomicBool::new(false);

    struct Hosted(Arc<Service>);

    impl WorkCloudSession for Hosted {
        fn signed_in(&self) -> bool {
            self.0.signed_in()
        }
        fn plan(&self) -> Option<String> {
            self.0.plan()
        }
        fn base(&self) -> String {
            self.0.origin()
        }
        fn credential(&self) -> Arc<dyn LeadCredential> {
            Arc::new(Bearer(self.0.clone()))
        }
    }

    struct Bearer(Arc<Service>);

    impl LeadCredential for Bearer {
        fn secret(&self) -> LeadSecretFuture<'_> {
            let service = self.0.clone();
            Box::pin(async move {
                // The first read after launch may touch the vault.
                let token = tokio::task::spawn_blocking(move || service.bearer())
                    .await
                    .map_err(|_| WorkModelError::MissingKey)?
                    .map_err(|_| WorkModelError::MissingKey)?;
                LeadSecret::new(token.expose().to_owned())
                    .map(Arc::new)
                    .map_err(|_| WorkModelError::MissingKey)
            })
        }

        fn rejected(&self) {
            self.0.bearer_refused();
        }
    }

    pub(super) fn install(service: Arc<Service>) {
        SIGNED_IN.store(service.signed_in(), Ordering::Release);
        work_models::install_cloud_session(Arc::new(Hosted(service)));
        refresh_catalog();
    }

    /// Only a sign-in or sign-out changes what the picker offers.
    pub(super) fn account_changed() {
        let Some(service) = super::service() else {
            return;
        };
        let now = service.signed_in();
        if SIGNED_IN.swap(now, Ordering::AcqRel) != now {
            work_models::cloud_session_changed();
            refresh_catalog();
        }
    }

    fn refresh_catalog() {
        tauri::async_runtime::spawn(work_models::refresh_cloud_catalog());
    }
}

pub(crate) fn service() -> Option<Arc<Service>> {
    SERVICE.get().cloned()
}

fn available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows")) && SERVICE.get().is_some()
}

fn project(view: View) -> AccountV1 {
    let config = view.config.as_ref();
    let billing = view.billing.as_ref();
    // Until Zephium's own server has been asked, what it offers is known.
    let ours = (!view.custom_server).then_some(true);
    AccountV1 {
        version: 1,
        available: available(),
        server: view.server,
        custom_server: view.custom_server,
        hosted_models: config.map(|c| c.cloud_enabled).or(ours),
        billing: config.map(|c| c.billing_enabled).or(ours),
        signup_open: config.map(|c| c.signup_open).or(ours),
        signed_in: view.signed_in,
        pending_email: view.pending_email,
        resend_in: view.resend_in,
        email: view.profile.as_ref().map(|p| p.email.clone()),
        name: view.profile.as_ref().and_then(|p| p.name.clone()),
        plan: view.usage.as_ref().and_then(|u| u.plan.clone()),
        usage: view.usage.as_ref().map(|usage| AccountUsageV1 {
            used_percent: usage.used_percent(),
            resets_at: usage.period_end.clone(),
            extra: usage.has_extra(),
        }),
        subscription: billing.map(|billing| AccountSubscriptionV1 {
            status: billing.subscription.status.clone(),
            interval: billing.subscription.interval.clone(),
            paid_until: billing.subscription.paid_until.clone(),
            cancel_at_period_end: billing.subscription.cancel_at_period_end,
        }),
        offers: billing.map(offers).unwrap_or_default(),
        checkout_open: view.checkout_open,
        notice: config
            .and_then(|c| c.notice.as_ref())
            .map(|notice| AccountNoticeV1 {
                id: notice.id.clone(),
                text: notice.text.clone(),
                warning: notice.severity.as_deref() == Some("warning"),
            }),
    }
}

fn price(price: &Option<zephium_account::wire::Price>) -> Option<AccountPriceV1> {
    let price = price.as_ref()?;
    Some(AccountPriceV1 {
        amount_minor: u32::try_from(price.amount_minor).ok()?,
        currency: price.currency.clone(),
    })
}

fn offers(billing: &Billing) -> Vec<AccountOfferV1> {
    let Some(offers) = &billing.offers else {
        return Vec::new();
    };
    let plans = offers
        .subscriptions
        .iter()
        .map(|offer| AccountOfferV1::Plan {
            plan: offer.plan.clone(),
            interval: offer.interval.clone(),
            name: offer.product_name.clone(),
            price: price(&offer.price),
            features: offer.features.clone(),
        });
    let topups = offers.topups.iter().map(|offer| AccountOfferV1::TopUp {
        id: offer.id.clone(),
        price: price(&offer.price),
        expires_days: offer.expires_days,
    });
    plans.chain(topups).collect()
}

fn unavailable_view() -> AccountV1 {
    AccountV1 {
        version: 1,
        available: false,
        server: zephium_account::DEFAULT_ORIGIN.to_owned(),
        custom_server: false,
        hosted_models: None,
        billing: None,
        signup_open: None,
        signed_in: false,
        pending_email: None,
        resend_in: None,
        email: None,
        name: None,
        plan: None,
        usage: None,
        subscription: None,
        offers: Vec::new(),
        checkout_open: false,
        notice: None,
    }
}

fn reply(result: Result<(), AccountError>) -> AccountReplyV1 {
    let account = service()
        .map(|service| project(service.view()))
        .unwrap_or_else(unavailable_view);
    let (fault, retry_after) = match result {
        Ok(()) => (None, None),
        Err(AccountError::RateLimited { retry_after }) => {
            (Some(AccountFaultV1::RateLimited), Some(retry_after))
        }
        Err(error) => (Some(error.into()), None),
    };
    AccountReplyV1 {
        account,
        fault,
        retry_after,
    }
}

fn admitted(caller: &WebviewWindow, label: &str) -> Result<Arc<Service>, AccountError> {
    if !authorize(caller, CallerPolicy::Main, label) {
        return Err(AccountError::Invalid);
    }
    service().ok_or(AccountError::Unavailable)
}

fn log(action: &str, result: &Result<(), AccountError>) {
    if let Err(error) = result {
        write_diagnostic(format_args!("account: action={action} fault={error:?}"));
    }
}

/// The account now, refreshing what is stale in the background. Asks no
/// server unless the person is signed in or chose one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn account_get(caller: WebviewWindow) -> AccountReplyV1 {
    let service = match admitted(&caller, "account_get") {
        Ok(service) => service,
        Err(error) => return reply(Err(error)),
    };
    if contacted(&service) {
        let background = service.clone();
        tauri::async_runtime::spawn(async move {
            if background.view().config.is_none() {
                let _ = background.load_config().await;
            }
            let _ = background.refresh(false).await;
        });
    }
    reply(Ok(()))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_send_code(caller: WebviewWindow, email: String) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_send_code") {
        Ok(_) if email.len() > MAX_EMAIL_INPUT_BYTES => Err(AccountError::Invalid),
        Ok(service) => service.send_code(&email).await,
        Err(error) => Err(error),
    };
    log("send_code", &result);
    reply(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_verify(caller: WebviewWindow, code: String) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_verify") {
        Ok(_) if code.len() > 16 => Err(AccountError::InvalidCode),
        Ok(service) => service.verify(&code).await,
        Err(error) => Err(error),
    };
    log("verify", &result);
    reply(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn account_cancel_code(caller: WebviewWindow) -> AccountReplyV1 {
    reply(admitted(&caller, "account_cancel_code").map(|service| service.cancel_code()))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_sign_out(caller: WebviewWindow) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_sign_out") {
        Ok(service) => service.sign_out().await,
        Err(error) => Err(error),
    };
    log("sign_out", &result);
    reply(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_refresh(caller: WebviewWindow) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_refresh") {
        Ok(service) => {
            let _ = service.load_config().await;
            service.refresh(true).await
        }
        Err(error) => Err(error),
    };
    log("refresh", &result);
    reply(result)
}

/// Opens the billing provider's checkout in a new tab and watches for the
/// purchase to land, so the plan updates without the person coming back.
#[tauri::command]
#[specta::specta]
pub(crate) async fn account_buy(
    caller: WebviewWindow,
    purchase: AccountPurchaseV1,
) -> AccountReplyV1 {
    let purchase = match purchase {
        AccountPurchaseV1::Plan { plan, interval } => Purchase::Plan { plan, interval },
        AccountPurchaseV1::TopUp { id } => Purchase::TopUp { id },
    };
    let result = match admitted(&caller, "account_buy") {
        Ok(service) => match service.checkout(&purchase).await {
            Ok(url) => {
                open_tab(&caller, url.as_str());
                watch_checkout(service);
                Ok(())
            }
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    log("buy", &result);
    reply(result)
}

/// Opens the billing provider's portal, where the subscription is managed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn account_manage(caller: WebviewWindow) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_manage") {
        Ok(service) => match service.portal().await {
            Ok(url) => {
                open_tab(&caller, url.as_str());
                Ok(())
            }
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    log("manage", &result);
    reply(result)
}

fn open_tab(caller: &WebviewWindow, url: &str) {
    let app = caller.app_handle();
    let shell = app.state::<Handle>();
    let _ = dispatch_operation(
        app,
        &shell,
        Command::OpenUrl {
            input: url.to_owned(),
            new_tab: true,
        },
    );
}

fn watch_checkout(service: Arc<Service>) {
    static WATCHING: AtomicBool = AtomicBool::new(false);
    if WATCHING.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        while service.checkout_open() {
            tokio::time::sleep(CHECKOUT_POLL).await;
            match service.refresh_billing().await {
                Err(AccountError::SignedOut) => break,
                Err(AccountError::RateLimited { retry_after }) => {
                    tokio::time::sleep(std::time::Duration::from_secs(retry_after.into())).await;
                }
                _ => {}
            }
        }
        WATCHING.store(false, Ordering::Release);
    });
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_sessions(caller: WebviewWindow) -> AccountSessionsV1 {
    let result = match admitted(&caller, "account_sessions") {
        Ok(service) => service.sessions().await,
        Err(error) => Err(error),
    };
    match result {
        Ok(sessions) => AccountSessionsV1 {
            sessions: sessions
                .into_iter()
                .map(|session| AccountSessionV1 {
                    id: session.id,
                    device: session.device,
                    created_at: session.created_at,
                    last_used_at: session.updated_at,
                    current: session.current,
                })
                .collect(),
            fault: None,
        },
        Err(error) => AccountSessionsV1 {
            sessions: Vec::new(),
            fault: Some(error.into()),
        },
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_revoke_session(caller: WebviewWindow, id: String) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_revoke_session") {
        Ok(_) if id.len() > 128 => Err(AccountError::Invalid),
        Ok(service) => service.revoke_session(&id).await,
        Err(error) => Err(error),
    };
    log("revoke_session", &result);
    reply(result)
}

/// Points accounts at another server, or back at Zephium's with null.
#[tauri::command]
#[specta::specta]
pub(crate) async fn account_set_server(
    caller: WebviewWindow,
    server: Option<String>,
) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_set_server") {
        Ok(_)
            if server
                .as_ref()
                .is_some_and(|s| s.len() > MAX_SERVER_INPUT_BYTES) =>
        {
            Err(AccountError::Invalid)
        }
        Ok(service) => match service.set_server(server.as_deref()).await {
            Ok(()) => {
                let _ = service.load_config().await;
                Ok(())
            }
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    log("set_server", &result);
    reply(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_deletion_code(caller: WebviewWindow) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_deletion_code") {
        Ok(service) => service.send_deletion_code().await,
        Err(error) => Err(error),
    };
    log("deletion_code", &result);
    reply(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_delete(caller: WebviewWindow, code: String) -> AccountReplyV1 {
    let result = match admitted(&caller, "account_delete") {
        Ok(_) if code.len() > 16 => Err(AccountError::InvalidCode),
        Ok(service) => service.delete_account(&code).await,
        Err(error) => Err(error),
    };
    log("delete", &result);
    reply(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faults_and_offers_have_stable_wire_shapes() {
        assert_eq!(
            serde_json::to_value(AccountFaultV1::NoBillingAccount).unwrap(),
            "no_billing_account"
        );
        let offer = AccountOfferV1::TopUp {
            id: "small".into(),
            price: Some(AccountPriceV1 {
                amount_minor: 500,
                currency: "usd".into(),
            }),
            expires_days: Some(365),
        };
        assert_eq!(
            serde_json::to_value(offer).unwrap(),
            serde_json::json!({
                "kind": "top_up", "id": "small",
                "price": {"amount_minor": 500, "currency": "usd"},
                "expires_days": 365
            })
        );
        let purchase: AccountPurchaseV1 = serde_json::from_value(
            serde_json::json!({"kind": "plan", "plan": "pro", "interval": "yearly"}),
        )
        .unwrap();
        assert!(matches!(purchase, AccountPurchaseV1::Plan { .. }));
    }
}
