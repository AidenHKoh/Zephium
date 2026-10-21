<script lang="ts">
  import type { Component } from "svelte";
  import {
    accountSession as session,
    creditsLeft,
    creditsTotal,
    planName,
    positive,
    shortDate,
    usageTone,
  } from "$domain/account";
  import { preferences } from "$domain/preferences";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import SettingsRow from "$shared/ui/SettingsRow";
  import { commands } from "$shared/ipc/bindings";
  import { explainAccountFault, SUPPORT_MAIL } from "../../lib/account";
  import { canChangePlan, intervalsOf, planCards, planOffers, topUpOffers } from "../../lib/plans";
  import type AccountCredits from "./AccountCredits.svelte";
  import AccountDailyLimit from "./AccountDailyLimit.svelte";
  import type AccountPlans from "./AccountPlans.svelte";

  const account = $derived(session.account());
  const busy = $derived(session.pending());
  const fault = $derived(session.fault());
  const usage = $derived(account?.usage ?? null);
  const subscription = $derived(account?.subscription ?? null);
  const paid = $derived(subscription?.status === "active");
  const billing = $derived(account?.billing === true);
  const hosted = $derived(account?.hosted_models !== false);
  /** Under billing review nothing new is bought; export, the portal and downgrades remain. */
  const review = $derived(account?.billing_review === true);
  const change = $derived(account?.plan_change ?? null);
  // Off, the plan stays and Zephium AI's usage and credits leave with Work.
  const ai = $derived(preferences.aiEnabled());
  /** Credits exist only where the server states how many units make one. */
  const per = $derived(usage?.credit_units ?? null);

  const plans = $derived(planOffers(account?.offers ?? []));
  const topups = $derived(topUpOffers(account?.offers ?? []));
  const currentPlan = $derived(account?.plan ?? null);
  /** A subscription changes plan, not billing interval; a first purchase picks one. */
  const subscribed = $derived(paid ? (subscription?.plan ?? null) : null);
  const held = $derived(paid ? (subscription?.interval ?? null) : null);
  const settled = $derived(!change && billing);
  const canPlans = $derived(
    settled &&
      canChangePlan(
        planCards({
          plans,
          currentPlan,
          subscribed,
          paid,
          interval: held ?? intervalsOf(plans)[0] ?? "monthly",
          review,
        }),
      ),
  );
  const canCredits = $derived(settled && ai && !review && topups.length > 0);
  /** The dialogs load when first asked for; most visits never open them. */
  let Plans = $state.raw<typeof AccountPlans | null>(null);
  let Credits = $state.raw<typeof AccountCredits | null>(null);
  let plansOpen = $state(false);
  let creditsOpen = $state(false);

  const usageLine = $derived.by(() => {
    const percent = usage?.used_percent;
    if (percent == null) return usage ? m.account_usage_none() : m.account_usage_loading();
    const left = creditsLeft(usage?.allowance_left, per);
    return left
      ? m.account_usage_left({ amount: left, percent })
      : m.account_usage_used_nodate({ percent });
  });
  /** A trial says it is one; a plan's allowance says when it refills. */
  const renewalLine = $derived.by(() => {
    if (usage?.used_percent == null) return null;
    if (usage.one_time) return m.account_usage_trial();
    const date = shortDate(usage.resets_at);
    return date ? m.account_usage_refills({ date }) : null;
  });
  const allowanceLine = $derived.by(() => {
    const used = creditsTotal(usage?.allowance_used ?? "0", per);
    const total = creditsLeft(usage?.allowance_total, per);
    return used && total && positive(usage?.allowance_total)
      ? m.account_allowance_value({ used, total })
      : null;
  });
  const purchased = $derived(creditsLeft(usage?.purchased_remaining, per));
  const hasPurchased = $derived(positive(usage?.purchased_remaining) && !!purchased);
  const pausedLine = $derived.by(() => {
    if (!usage?.free_paused) return null;
    return hasPurchased && usage.purchased_usable !== false
      ? m.account_usage_paused_purchased()
      : m.account_usage_paused();
  });
  const bonusLine = $derived.by(() => {
    if (!positive(usage?.bonus_remaining)) return null;
    const left = creditsLeft(usage?.bonus_remaining, per);
    if (!left) return null;
    const date = shortDate(usage?.bonus_expires_at);
    return date
      ? m.account_bonus_value({ amount: left, date })
      : m.account_bonus_value_nodate({ amount: left });
  });
  const heldCredits = $derived(positive(usage?.held) ? creditsTotal(usage?.held, per) : null);
  const planLine = $derived.by(() => {
    if (!subscription || subscription.status === "inactive") return undefined;
    if (subscription.status === "unknown") return m.account_plan_unknown();
    const date = shortDate(subscription.paid_until);
    if (!date) return m.account_plan_active();
    return subscription.cancel_at_period_end
      ? m.account_plan_ends({ date })
      : m.account_plan_renews({ date });
  });
  const unconfirmed = $derived(
    (account?.unsettled ?? []).some((request) => request.state !== "settled"),
  );

  async function showPlans() {
    Plans ??= (await import("./AccountPlans.svelte")).default;
    plansOpen = true;
  }

  async function showCredits() {
    Credits ??= (await import("./AccountCredits.svelte")).default;
    creditsOpen = true;
  }

  /** Usage history is loaded only when someone asks to see it. */
  let History = $state.raw<Component | null>(null);
  let loadingHistory = $state(false);
  async function showHistory() {
    loadingHistory = true;
    try {
      History = (await import("./AccountHistory.svelte")).default;
    } finally {
      loadingHistory = false;
    }
  }

  /** A change in progress, or a quote waiting to be confirmed, is shown where it is read. */
  let Change = $state.raw<Component | null>(null);
  $effect(() => {
    if (!change || Change) return;
    void import("./AccountPlanChange.svelte").then((module) => (Change = module.default));
  });

  const billingFault = $derived(
    fault && ["buy", "manage", "plan_preview"].includes(fault.action)
      ? explainAccountFault(fault.fault, fault.retryAfter)
      : null,
  );
</script>

{#if !hosted}{#if ai}<p class="note">{m.account_hosted_off()}</p>{/if}
{:else}
  <section class="plan" aria-label={ai ? m.account_usage_group() : m.account_plan_group()}>
    <div class="head">
      <div class="title">
        <span class="eyebrow">{ai ? m.account_usage_group() : m.account_plan_group()}</span>
        <h3>{planName(currentPlan) ?? m.account_plan_free()}</h3>
        {#if planLine}<p>{planLine}</p>{/if}
      </div>
      {#if billing && (paid || subscription?.status === "unknown")}<Button
          pending={busy === "manage"}
          disabled={!!busy && busy !== "manage"}
          onclick={() => void session.manage()}>{m.account_billing_manage()}</Button
        >{/if}
    </div>
    {#if ai}<div class="usage">
        <div class="usage-copy">
          <h4>{m.account_usage_title()}</h4>
          <p>{usageLine}</p>
        </div>
        {#if usage?.used_percent != null}<div
            class="meter"
            role="meter"
            aria-label={m.account_usage_meter()}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={usage.used_percent}
            aria-valuetext={usageLine}
            data-tone={usageTone(usage.used_percent)}
          >
            <span style:inline-size={`${usage.used_percent}%`}></span>
          </div>
          {#if renewalLine || allowanceLine}<p class="caption">
              {[renewalLine, allowanceLine].filter(Boolean).join(" · ")}
            </p>{/if}{/if}
        {#if pausedLine}<p class="caption" role="status">{pausedLine}</p>{/if}
      </div>{/if}
    {#if canPlans || canCredits}<div class="actions">
        {#if canCredits}<Button disabled={!!busy} onclick={() => void showCredits()}
            >{m.account_credits_open()}</Button
          >{/if}
        {#if canPlans}<Button
            variant={paid ? "secondary" : "primary"}
            disabled={!!busy}
            onclick={() => void showPlans()}
            >{paid ? m.account_plans_change() : m.account_upgrade()}</Button
          >{/if}
      </div>{/if}
    {#if review}<div class="review" role="status">
        <h4>{m.account_review_title()}</h4>
        <p>
          {m.account_review_body()}
          <a
            href={SUPPORT_MAIL}
            onclick={(event) => {
              event.preventDefault();
              void commands.browserOpenUrl(SUPPORT_MAIL, true).catch(() => {});
            }}>{m.account_review_contact()}</a
          >.
        </p>
      </div>{/if}
    {#if Change && change}<Change />{/if}
    {#if ai}<div class="rows">
        {#if bonusLine}<SettingsRow title={m.account_bonus_title()}
            ><span class="value">{bonusLine}</span></SettingsRow
          >{/if}
        {#if per}<SettingsRow title={m.account_extra_title()}
            ><span class="value"
              >{hasPurchased
                ? m.account_extra_value({ amount: purchased ?? "0" })
                : m.account_extra_none()}</span
            ></SettingsRow
          >{/if}
        {#if heldCredits}<SettingsRow title={m.account_held_title()}
            ><span class="value">{m.account_held_value({ amount: heldCredits })}</span></SettingsRow
          >{/if}
        <AccountDailyLimit />
        {#if History}<History />
        {:else}<SettingsRow
            title={m.account_history_row()}
            description={m.account_history_row_desc()}
            ><Button size="compact" pending={loadingHistory} onclick={() => void showHistory()}
              >{m.account_history_show()}</Button
            ></SettingsRow
          >{/if}
      </div>{/if}
  </section>
  {#if canPlans && Plans}<Plans
      bind:open={plansOpen}
      {plans}
      {currentPlan}
      {subscribed}
      {paid}
      {held}
      {review}
    />{/if}
  {#if canCredits && Credits}<Credits bind:open={creditsOpen} packs={topups} {per} />{/if}
  {#if unconfirmed}<p class="note" role="status">{m.account_unsettled()}</p>{/if}
  {#if billing && account?.checkout_open}<p class="note" role="status">
      {account.portal_open ? m.account_portal_waiting() : m.account_checkout_waiting()}
    </p>{/if}
  {#if billingFault}<p class="note" data-tone="error" role="alert">{billingFault}</p>{/if}
{/if}

<style>
  /* The plan is the account's one card: what it is, how much of it is used,
     and the way to more, before the details under it. */
  .plan {
    overflow: hidden;
    margin-block-end: 34px;
    border-radius: var(--radius-card);
    background: var(--color-surface);
    box-shadow: var(--shadow-card);
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 20px 18px 0;
  }

  .title {
    display: grid;
    gap: 2px;
    min-inline-size: 0;
  }

  .eyebrow {
    color: var(--color-muted);
    font-size: var(--text-caption);
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  h3 {
    margin: 0;
    color: var(--color-text);
    font-size: 20px;
    font-weight: 600;
    line-height: 1.3;
    letter-spacing: -0.02em;
  }

  .title p {
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .usage {
    display: grid;
    gap: 10px;
    padding: 18px 18px 20px;
  }

  .usage-copy {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 4px 16px;
  }

  h4 {
    margin: 0;
    color: var(--color-text);
    font-size: var(--text-page-title);
    font-weight: 500;
    line-height: 19px;
  }

  .usage-copy p,
  .value,
  .caption {
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
  }

  .meter {
    overflow: hidden;
    block-size: 8px;
    border-radius: var(--radius-capsule);
    background: var(--color-track);
  }

  .meter span {
    display: block;
    block-size: 100%;
    border-radius: inherit;
    background: var(--color-text);
    transition: inline-size var(--motion-slow) var(--ease-out);
  }

  .meter[data-tone="high"] span {
    background: var(--color-warning);
  }

  .meter[data-tone="full"] span {
    background: var(--color-danger);
  }

  .actions {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px;
    padding: 0 18px 20px;
  }

  .head + .actions {
    padding-block-start: 18px;
  }

  .review {
    display: grid;
    gap: 4px;
    margin: 0 18px 18px;
    padding: 14px 16px;
    border-radius: var(--radius-control);
    background: var(--color-fill);
  }

  .review h4 {
    font-size: var(--text-body);
  }

  .review p {
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 1.5;
  }

  .rows {
    position: relative;
  }

  .rows::before {
    position: absolute;
    top: 0;
    block-size: 1px;
    background: var(--color-border);
    content: "";
    inset-inline: 16px 0;
  }

  .note {
    margin: -22px 18px 30px;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 1.5;
  }

  .note[data-tone="error"] {
    color: var(--color-danger);
  }

  @media (prefers-reduced-motion: reduce) {
    .meter span {
      transition: none;
    }
  }
</style>
