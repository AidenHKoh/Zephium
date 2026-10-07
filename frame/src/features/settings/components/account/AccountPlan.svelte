<script lang="ts">
  import {
    accountSession as session,
    offerKey,
    planName,
    priceLabel,
    shortDate,
    usageTone,
  } from "$domain/account";
  import type { AccountOfferV1 } from "$shared/ipc/bindings";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import SegmentedControl from "$shared/ui/SegmentedControl";
  import SettingsGroup from "$shared/ui/SettingsGroup";
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

  const usageLine = $derived.by(() => {
    const percent = usage?.used_percent;
    if (percent == null) return usage ? m.account_usage_none() : m.account_usage_loading();
    const date = shortDate(usage?.resets_at);
    return date
      ? m.account_usage_used({ percent, date })
      : m.account_usage_used_nodate({ percent });
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

{#if !hosted}<p class="note">{m.account_hosted_off()}</p>
{:else}
  <SettingsGroup title={m.account_usage_group()}>
    <div class="usage">
      <div class="usage-copy">
        <h3>{m.account_usage_title()}</h3>
        <p>{usageLine}</p>
      </div>
      {#if usage?.used_percent != null}<div
          class="meter"
          role="meter"
          aria-label={m.account_usage_meter()}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={usage.used_percent}
          data-tone={usageTone(usage.used_percent)}
        >
          <span style:inline-size={`${usage.used_percent}%`}></span>
        </div>{/if}
    </div>
    <SettingsRow title={m.account_extra_title()}
      ><span class="value">{usage?.extra ? m.account_extra_left() : m.account_extra_none()}</span
      ></SettingsRow
    >
  </SettingsGroup>

  {#if billing}
    <SettingsGroup title={m.account_plan_group()}>
      <SettingsRow title={planName(currentPlan) ?? m.account_plan_free()} description={planLine}>
        {#if paid}<Button
            pending={busy === "manage"}
            disabled={!!busy && busy !== "manage"}
            onclick={() => void session.manage()}>{m.account_billing_manage()}</Button
          >{/if}
      </SettingsRow>
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
      {#each topups as offer (offer.id)}
        {@const price = priceLabel(offer.price)}
        <SettingsRow
          title={m.account_topup_title()}
          description={offer.expires_days
            ? m.account_topup_desc_days({ days: offer.expires_days })
            : m.account_topup_desc()}
        >
          <Button
            pending={buying === offerKey(offer)}
            disabled={!!busy}
            onclick={() => void buy(offer)}>{price ?? m.account_topup_buy()}</Button
          >
        </SettingsRow>
      {/each}
    </SettingsGroup>
    {#if account?.checkout_open}<p class="note" role="status">
        {m.account_checkout_waiting()}
      </p>{/if}
    {#if billingFault}<p class="note" data-tone="error" role="alert">{billingFault}</p>{/if}
  {/if}
{/if}

<style>
  .usage {
    display: grid;
    gap: 12px;
    padding: 14px 18px;
  }

  .usage-copy h3 {
    margin: 0;
    color: var(--color-text);
    font-size: var(--text-page-title);
    font-weight: 500;
    line-height: 19px;
  }

  .usage-copy p,
  .value {
    margin: 2px 0 0;
    color: var(--color-muted);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
  }

  .meter {
    overflow: hidden;
    block-size: 6px;
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
