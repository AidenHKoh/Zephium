//! What the account service answers, read leniently: a newer server may add
//! fields, and a self-hosted one may leave optional ones out.
//!
//! Amounts stay decimal strings of integer units, as the server sends them.

use serde::{Deserialize, Serialize};

/// One display credit, in the integer units every amount is counted in.
pub const UNIT_SCALE: u64 = 1_000_000;

/// `GET /v1/client-config`: what this server offers, before signing in.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ClientConfig {
    #[serde(default)]
    pub cloud_enabled: bool,
    #[serde(default = "yes")]
    pub signup_open: bool,
    #[serde(default)]
    pub billing_enabled: bool,
    #[serde(default)]
    pub min_client_version: Option<String>,
    #[serde(default)]
    pub notice: Option<Notice>,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Notice {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub severity: Option<String>,
}

/// `GET /v1/account`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "createdAt", alias = "created_at")]
    pub created_at: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Name {
    pub(crate) name: Option<String>,
}

/// One row of `GET /v1/sessions`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub current: bool,
}

#[derive(Deserialize)]
pub(crate) struct Sessions {
    pub(crate) sessions: Vec<Session>,
}

/// `GET /v1/billing`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Billing {
    #[serde(default)]
    pub subscription: Subscription,
    #[serde(default)]
    pub paid: bool,
    #[serde(default)]
    pub capabilities: Capabilities,
    /// Null when this server sells nothing.
    #[serde(default)]
    pub offers: Option<Offers>,
}

impl Billing {
    /// Whether anything can be bought or managed here. A server with billing
    /// off still reports a subscription, as `not_applicable`.
    pub fn sells(&self) -> bool {
        self.offers.is_some() && self.subscription.status != "not_applicable"
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Capabilities {
    #[serde(default, rename = "hostedAI", alias = "hosted_ai")]
    pub hosted_ai: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Subscription {
    /// `active`, `inactive`, `unknown` (not reconciled lately), or
    /// `not_applicable` when the server has billing off.
    #[serde(default)]
    pub status: String,
    #[serde(default, rename = "paidUntil", alias = "paid_until")]
    pub paid_until: Option<String>,
    #[serde(default, rename = "cancelAtPeriodEnd", alias = "cancel_at_period_end")]
    pub cancel_at_period_end: bool,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub interval: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Offers {
    #[serde(default)]
    pub subscriptions: Vec<PlanOffer>,
    #[serde(default)]
    pub topups: Vec<TopUpOffer>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PlanOffer {
    pub plan: String,
    pub interval: String,
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub price: Option<Price>,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TopUpOffer {
    pub id: String,
    #[serde(default)]
    pub units: Option<String>,
    #[serde(default)]
    pub expires_days: Option<u32>,
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub price: Option<Price>,
}

/// A price as the billing provider states it, in the currency's minor unit.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Price {
    pub amount_minor: u64,
    pub currency: String,
}

/// `GET /v1/usage`. Every amount is a decimal string of integer units.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Usage {
    #[serde(default)]
    pub cloud_enabled: bool,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub period_end: Option<String>,
    #[serde(default)]
    pub allowance: Allowance,
    #[serde(default)]
    pub purchased_remaining: Option<String>,
    #[serde(default)]
    pub unit_scale: Option<u64>,
    #[serde(default)]
    pub credits_remaining: Option<String>,
    /// Held for calls still running; never a settled charge.
    #[serde(default)]
    pub reserved_units: Option<String>,
    #[serde(default)]
    pub spent_today: Option<String>,
    #[serde(default)]
    pub daily_limit_units: Option<String>,
    #[serde(default)]
    pub bonus_remaining: Option<String>,
    #[serde(default)]
    pub bonus_expires_at: Option<String>,
    #[serde(default)]
    pub by_model: Vec<ModelUsage>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Allowance {
    #[serde(default)]
    pub total_units: Option<String>,
    #[serde(default)]
    pub used_units: Option<String>,
    #[serde(default)]
    pub reserved_units: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModelUsage {
    pub model_id: String,
    #[serde(default)]
    pub units: Option<String>,
    #[serde(default)]
    pub requests: u64,
}

impl Usage {
    /// Share of the plan's allowance used or held, 0–100. None without an allowance.
    pub fn used_percent(&self) -> Option<u8> {
        let total = units(self.allowance.total_units.as_deref())?;
        if total == 0 {
            return None;
        }
        let spent = units(self.allowance.used_units.as_deref()).unwrap_or(0)
            + units(self.allowance.reserved_units.as_deref()).unwrap_or(0);
        Some((spent.min(total) * 100 / total) as u8)
    }

    /// Whether bought usage remains beyond the plan's allowance.
    pub fn has_extra(&self) -> bool {
        units(self.purchased_remaining.as_deref()).is_some_and(|units| units > 0)
    }

    /// The scale amounts are counted in; the contract fixes it, so anything
    /// else is read as the contract's.
    pub fn scale(&self) -> u64 {
        self.unit_scale
            .filter(|scale| *scale > 0)
            .unwrap_or(UNIT_SCALE)
    }
}

/// An amount as the server sent it, when it is a well-formed unit count.
pub fn amount(value: Option<&str>) -> Option<String> {
    units(value).map(|units| units.to_string())
}

fn units(value: Option<&str>) -> Option<u128> {
    let value = value?.trim();
    if value.is_empty() || value.len() > 38 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

/// One page of `GET /v1/usage/requests`.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct UsageHistory {
    #[serde(default)]
    pub requests: Vec<UsageRequest>,
    /// Opaque; passed back unchanged for the next page.
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UsageRequest {
    pub id: String,
    #[serde(default)]
    pub model_id: Option<String>,
    pub state: String,
    /// Charged so far; final only when `state` is `settled`.
    #[serde(default)]
    pub units: Option<String>,
    #[serde(default)]
    pub reserved: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

/// `GET /v1/usage/requests/{id}`: one call's accounting, never its content.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Receipt {
    pub id: String,
    pub state: String,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub policy_revision: Option<u64>,
    #[serde(default)]
    pub reserved: Option<String>,
    #[serde(default)]
    pub charged: Option<String>,
    #[serde(default)]
    pub error_class: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

impl Receipt {
    /// Only a settled request has a final charge.
    pub fn settled(&self) -> bool {
        self.state == "settled"
    }
}

/// What the person picked to buy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Purchase {
    Plan { plan: String, interval: String },
    TopUp { id: String },
}

#[derive(Deserialize)]
pub(crate) struct Link {
    pub(crate) url: String,
}

#[derive(Deserialize)]
pub(crate) struct ErrorBody {
    pub(crate) error: ErrorCode,
}

#[derive(Deserialize)]
pub(crate) struct ErrorCode {
    pub(crate) code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_percent_counts_holds_and_never_exceeds_the_allowance() {
        let mut usage: Usage = serde_json::from_value(serde_json::json!({
            "allowance": {"total_units": "4000000", "used_units": "1000000", "reserved_units": "500000"},
            "purchased_remaining": "0"
        }))
        .unwrap();
        assert_eq!(usage.used_percent(), Some(37));
        assert!(!usage.has_extra());
        usage.allowance.used_units = Some("9000000".into());
        assert_eq!(usage.used_percent(), Some(100));
        usage.allowance.total_units = Some("0".into());
        assert_eq!(usage.used_percent(), None);
        usage.allowance.total_units = Some("-1".into());
        assert_eq!(usage.used_percent(), None);
    }

    #[test]
    fn usage_reads_the_full_contract_and_keeps_amounts_as_strings() {
        let usage: Usage = serde_json::from_value(serde_json::json!({
            "schema_version": 1, "cloud_enabled": true, "policy_revision": 3, "plan": "free",
            "unit_scale": 1_000_000, "credits_remaining": "2500000", "reserved_units": "120000",
            "spent_today": "300000", "bonus_remaining": "1000000",
            "bonus_expires_at": "2026-10-20T00:00:00.000Z", "purchased_remaining": "0",
            "period_end": "2026-10-16T00:00:00.000Z", "daily_limit_units": null,
            "by_model": [{"model_id": "fast", "units": "300000", "requests": 4}],
            "allowance": {"total_units": "2000000", "used_units": "300000", "reserved_units": "120000"}
        }))
        .unwrap();
        assert_eq!(usage.credits_remaining.as_deref(), Some("2500000"));
        assert_eq!(usage.bonus_remaining.as_deref(), Some("1000000"));
        assert_eq!(usage.daily_limit_units, None);
        assert_eq!(usage.by_model[0].requests, 4);
        assert_eq!(usage.scale(), UNIT_SCALE);
        assert_eq!(amount(Some("007")), Some("7".into()));
        assert_eq!(amount(Some("1.5")), None);
    }

    #[test]
    fn billing_reads_the_servers_camel_case_and_missing_offers() {
        let billing: Billing = serde_json::from_value(serde_json::json!({
            "subscription": {"status": "active", "paidUntil": "2026-11-01T00:00:00Z", "cancelAtPeriodEnd": true},
            "paid": true,
            "capabilities": {"hostedAI": true, "synchronization": false},
            "offers": null
        }))
        .unwrap();
        assert!(billing.paid);
        assert!(billing.capabilities.hosted_ai);
        assert!(billing.subscription.cancel_at_period_end);
        assert_eq!(billing.offers, None);
        assert!(!billing.sells());
    }

    #[test]
    fn billing_off_sells_nothing_even_with_an_empty_offer_list() {
        let billing: Billing = serde_json::from_value(serde_json::json!({
            "subscription": {"status": "not_applicable", "paidUntil": null, "cancelAtPeriodEnd": false},
            "paid": false,
            "capabilities": {"hostedAI": true, "synchronization": false},
            "offers": {"subscriptions": [], "topups": [], "unit_scale": 1_000_000}
        }))
        .unwrap();
        assert!(!billing.sells());
    }
}
