<script lang="ts">
  import { tick } from "svelte";
  import {
    accountSession as session,
    creditsLeft,
    creditsToUnits,
    creditsTotal,
    positive,
    unitsToCredits,
  } from "$domain/account";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Field from "$shared/ui/Field";
  import SettingsRow from "$shared/ui/SettingsRow";
  import { explainAccountFault } from "../../lib/account";

  const usage = $derived(session.account()?.usage ?? null);
  const per = $derived(usage?.credit_units ?? null);
  const busy = $derived(session.pending());
  const fault = $derived(session.fault());
  const limit = $derived(positive(usage?.daily_limit) ? (usage?.daily_limit ?? null) : null);
  let editing = $state(false);
  let value = $state("");
  let form = $state<HTMLFormElement | null>(null);

  const line = $derived.by(() => {
    const cap = creditsLeft(limit, per);
    if (!cap) return m.account_daily_none();
    return m.account_daily_value({
      spent: creditsTotal(usage?.spent_today ?? "0", per) ?? "0",
      limit: cap,
    });
  });
  const failure = $derived.by(() => {
    if (fault?.action !== "limit") return null;
    return fault.fault === "invalid"
      ? m.account_daily_invalid()
      : explainAccountFault(fault.fault, fault.retryAfter);
  });
  const units = $derived(creditsToUnits(value, per));

  async function edit() {
    value = unitsToCredits(limit, per) ?? "";
    editing = true;
    await tick();
    form?.querySelector("input")?.focus();
  }

  async function save() {
    if (!units || busy) return;
    if (await session.setDailyLimit(units)) editing = false;
  }

  async function remove() {
    if (busy) return;
    if (await session.setDailyLimit(null)) editing = false;
  }
</script>

{#if per}
  <SettingsRow title={m.account_daily_title()} description={m.account_daily_desc()}>
    <span class="value">{line}</span>
    {#if !editing}<Button size="compact" disabled={!!busy} onclick={() => void edit()}
        >{limit ? m.account_daily_change() : m.account_daily_set()}</Button
      >{/if}
  </SettingsRow>
  {#if editing}
    <form
      class="edit"
      bind:this={form}
      onsubmit={(event) => {
        event.preventDefault();
        void save();
      }}
    >
      <Field
        label={m.account_daily_field()}
        inputmode="numeric"
        autocomplete="off"
        maxlength={9}
        bind:value
        onkeydown={(event) => {
          if (event.key === "Escape") editing = false;
        }}
      />
      <div class="actions">
        {#if limit}<Button
            size="compact"
            variant="ghost"
            pending={busy === "limit"}
            disabled={!!busy}
            onclick={() => void remove()}>{m.account_daily_remove()}</Button
          >{/if}
        <Button size="compact" variant="ghost" onclick={() => (editing = false)}
          >{m.action_cancel()}</Button
        >
        <Button size="compact" type="submit" variant="primary" disabled={!units || !!busy}
          >{m.account_daily_save()}</Button
        >
      </div>
    </form>
  {/if}
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
{/if}

<style>
  .value {
    color: var(--color-muted);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
  }

  .edit {
    display: grid;
    gap: 10px;
    padding: 0 18px 14px;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .error {
    margin: 0;
    padding: 0 18px 12px;
    color: var(--color-danger);
    font-size: var(--text-label);
    line-height: 1.5;
  }
</style>
