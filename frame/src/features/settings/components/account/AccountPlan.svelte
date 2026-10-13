<script lang="ts">
  import type { Component } from "svelte";
  import {
    accountSession as session,
    credits,
    offerKey,
    planName,
    positive,
    priceLabel,
    shortDate,
    usageTone,
  } from "$domain/account";
  import type { AccountOfferV1 } from "$shared/ipc/bindings";
  import { preferences } from "$domain/preferences";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import SegmentedControl from "$shared/ui/SegmentedControl";
  import SettingsRow from "$shared/ui/SettingsRow";
  import { explainAccountFault } from "../../lib/account";

  const account = $derived(session.account());
  const busy = $derived(session.pending());
  const fault = $derived(session.fault());
  const usage = $derived(account?.usage ?? null);
  const subscription = $derived(account?.subscription ?? null);
  const paid = $derived(subscription?.status === "active");
  const billing = $derived(account?.billing === true);
  const hosted = $derived(account?.hosted_models !== false);
  // Off, the plan stays and Zephium AI's usage and credits leave with Work.
  const ai = $derived(preferences.aiEnabled());
  const amount = (units: string | null | undefined) =>
    usage ? credits(units, usage.unit_scale) : null;

  type Plan = Extract<AccountOfferV1, { kind: "plan" }>;
  type TopUp = Extract<AccountOfferV1, { kind: "top_up" }>;
  const plans = $derived(
    (account?.offers ?? []).filter((offer): offer is Plan => offer.kind === "plan"),
  );
  const topups = $derived(
    (account?.offers ?? []).filter((offer): offer is TopUp => offer.kind === "top_up"),
  );
  /** Each plan once, in the server's order, with the intervals it is sold in. */
  const planIds = $derived([...new Set(plans.map((plan) => plan.plan))]);
  const currentPlan = $derived(account?.plan ?? null);
  const upgrades = $derived(paid ? [] : planIds.filter((id) => id !== currentPlan));
  let interval = $state<string>("yearly");
  const intervals = $derived(
    [...new Set(plans.map((plan) => plan.interval))].sort(
      (a, b) => Number(b === "monthly") - Number(a === "monthly"),
    ),
  );
  $effect(() => {
    if (intervals.length && !intervals.includes(interval)) interval = intervals[0]!;
  });

  function offerFor(id: string): Plan | undefined {
    return (
      plans.find((plan) => plan.plan === id && plan.interval === interval) ??
      plans.find((plan) => plan.plan === id)
    );
  }

  function priced(plan: Plan): string | null {
    const price = priceLabel(plan.price);
    if (!price) return null;
    return plan.interval === "yearly"
      ? m.account_price_year({ price })
      : m.account_price_month({ price });
  }

  function intervalName(value: string) {
    if (value === "monthly") return m.account_interval_monthly();
    if (value === "yearly") return m.account_interval_yearly();
    return planName(value) ?? value;
  }

  function topUpLine(offer: TopUp) {
    const added = offer.units ? credits(offer.units, usage?.unit_scale ?? 1_000_000) : null;
    if (added && offer.expires_days)
      return m.account_topup_adds_days({ amount: added, days: offer.expires_days });
    if (added) return m.account_topup_adds({ amount: added });
    return offer.expires_days
      ? m.account_topup_desc_days({ days: offer.expires_days })
      : m.account_topup_desc();
  }

  const usageLine = $derived.by(() => {
    const percent = usage?.used_percent;
    if (percent == null) return usage ? m.account_usage_none() : m.account_usage_loading();
    const date = shortDate(usage?.resets_at);
    return date
      ? m.account_usage_used({ percent, date })
      : m.account_usage_used_nodate({ percent });
  });
  const allowanceLine = $derived.by(() => {
    const used = amount(usage?.allowance_used ?? "0");
    const total = amount(usage?.allowance_total);
    return used && total && positive(usage?.allowance_total)
      ? m.account_allowance_value({ used, total })
      : null;
  });
  const bonusLine = $derived.by(() => {
    if (!positive(usage?.bonus_remaining)) return null;
    const left = amount(usage?.bonus_remaining);
    if (!left) return null;
    const date = shortDate(usage?.bonus_expires_at);
    return date
      ? m.account_bonus_value({ amount: left, date })
      : m.account_bonus_value_nodate({ amount: left });
  });
  const held = $derived(positive(usage?.held) ? amount(usage?.held) : null);
  const dailyLine = $derived.by(() => {
    const limit = amount(usage?.daily_limit);
    if (!limit) return null;
    return m.account_daily_value({ spent: amount(usage?.spent_today ?? "0") ?? "0", limit });
  });
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

  const billingFault = $derived(
    fault && (fault.action === "buy" || fault.action === "manage")
      ? explainAccountFault(fault.fault, fault.retryAfter)
      : null,
  );
  let buying = $state<string | null>(null);

  async function buy(offer: AccountOfferV1) {
    buying = offerKey(offer);
    await session.buy(
      offer.kind === "plan"
        ? { kind: "plan", plan: offer.plan, interval: offer.interval }
        : { kind: "top_up", id: offer.id },
    );
    buying = null;
  }
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
      {#if billing && paid}<Button
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
            aria-valuetext={allowanceLine ?? undefined}
            data-tone={usageTone(usage.used_percent)}
          >
            <span style:inline-size={`${usage.used_percent}%`}></span>
          </div>
          {#if allowanceLine}<p class="caption">{allowanceLine}</p>{/if}{/if}
      </div>{/if}
    <div class="rows">
      {#if ai}{#if bonusLine}<SettingsRow title={m.account_bonus_title()}
            ><span class="value">{bonusLine}</span></SettingsRow
          >{/if}
        <SettingsRow title={m.account_extra_title()}
          ><span class="value"
            >{positive(usage?.purchased_remaining) && amount(usage?.purchased_remaining)
              ? m.account_extra_value({ amount: amount(usage?.purchased_remaining)! })
              : m.account_extra_none()}</span
          ></SettingsRow
        >
        {#if held}<SettingsRow title={m.account_held_title()}
            ><span class="value">{m.account_held_value({ amount: held })}</span></SettingsRow
          >{/if}
        {#if dailyLine}<SettingsRow title={m.account_daily_title()}
            ><span class="value">{dailyLine}</span></SettingsRow
          >{/if}
        {#if History}<History />
        {:else}<SettingsRow
            title={m.account_history_row()}
            description={m.account_history_row_desc()}
            ><Button size="compact" pending={loadingHistory} onclick={() => void showHistory()}
              >{m.account_history_show()}</Button
            ></SettingsRow
          >{/if}{/if}
      {#if billing}
        {#each upgrades as id (id)}
          {@const offer = offerFor(id)}
          {#if offer}<SettingsRow
              title={offer.name ?? planName(id) ?? id}
              description={[priced(offer), ...offer.features].filter(Boolean).join(" · ")}
            >
              {#if intervals.length > 1}<SegmentedControl
                  label={m.account_plan_group()}
                  size="compact"
                  options={intervals.map((value) => ({ value, label: intervalName(value) }))}
                  bind:value={interval}
                />{/if}
              <Button
                variant="primary"
                pending={buying === offerKey(offer)}
                disabled={!!busy}
                onclick={() => void buy(offer)}>{m.account_upgrade()}</Button
              >
            </SettingsRow>{/if}
        {/each}
        {#each ai ? topups : [] as offer (offer.id)}
          {@const price = priceLabel(offer.price)}
          <SettingsRow title={offer.name ?? m.account_topup_title()} description={topUpLine(offer)}>
            <Button
              pending={buying === offerKey(offer)}
              disabled={!!busy}
              onclick={() => void buy(offer)}>{price ?? m.account_topup_buy()}</Button
            >
          </SettingsRow>
        {/each}
      {/if}
    </div>
  </section>
  {#if unconfirmed}<p class="note" role="status">{m.account_unsettled()}</p>{/if}
  {#if billing && account?.checkout_open}<p class="note" role="status">
      {m.account_checkout_waiting()}
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
