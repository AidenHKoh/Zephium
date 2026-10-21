import type { AccountUsageV1, AccountV1 } from "$shared/ipc/bindings";

/** A signed-out account on Zephium's server, as `account_get` answers it. */
export function accountFixture(overrides: Partial<AccountV1> = {}): AccountV1 {
  return {
    version: 1,
    available: true,
    server: "https://api.zephium.app",
    custom_server: false,
    hosted_models: true,
    billing: true,
    signup_open: true,
    update_required: false,
    signed_in: false,
    pending_email: null,
    resend_in: null,
    email: null,
    name: null,
    avatar: null,
    plan: null,
    usage: null,
    subscription: null,
    offers: [],
    checkout_open: false,
    portal_open: false,
    signup_mode: null,
    billing_review: false,
    plan_change: null,
    notice: null,
    unsettled: [],
    ...overrides,
  };
}

/**
 * Usage in integer units, a million to the server's own unit and `credit_units`
 * (10,000, as staging sends it) to a credit: 100 credits to a dollar.
 */
export function usageFixture(overrides: Partial<AccountUsageV1> = {}): AccountUsageV1 {
  return {
    used_percent: 42,
    resets_at: "2026-10-14T00:00:00Z",
    extra: false,
    unit_scale: 1_000_000,
    remaining: "1160000",
    allowance_total: "2000000",
    allowance_used: "840000",
    held: null,
    bonus_remaining: null,
    bonus_expires_at: null,
    purchased_remaining: "0",
    spent_today: "840000",
    daily_limit: null,
    credit_units: "10000",
    allowance_left: "1160000",
    one_time: false,
    free_paused: false,
    purchased_usable: null,
    ...overrides,
  };
}
