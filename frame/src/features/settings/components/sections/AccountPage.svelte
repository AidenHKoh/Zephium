<script lang="ts">
  import { onMount } from "svelte";
  import { accountSession as session, initials, planName } from "$domain/account";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import AccountDelete from "../account/AccountDelete.svelte";
  import AccountDevices from "../account/AccountDevices.svelte";
  import AccountPlan from "../account/AccountPlan.svelte";
  import AccountServer from "../account/AccountServer.svelte";
  import AccountSignIn from "../account/AccountSignIn.svelte";
  import { explainAccountFault } from "../../lib/account";

  onMount(() => {
    void session.retain().then(() => session.refresh());
    return () => session.release();
  });

  const account = $derived(session.account());
  const busy = $derived(session.pending());
  const fault = $derived(session.fault());
  const ended = $derived(
    fault?.fault === "signed_out" ? explainAccountFault("signed_out", null) : null,
  );
  const subtitle = $derived.by(() => {
    if (!account) return "";
    const plan = planName(account.plan);
    return [account.name ? account.email : null, plan ? m.account_plan_line({ plan }) : null]
      .filter(Boolean)
      .join(" · ");
  });
  const signOutFault = $derived(
    fault?.action === "sign_out" ? explainAccountFault(fault.fault, fault.retryAfter) : null,
  );
</script>

{#if session.unavailable()}<p class="note" role="status">{m.account_unavailable()}</p>
{:else if account}
  {#if !account.signed_in}
    {#if ended}<p class="note" role="status">{ended}</p>{/if}
    <AccountSignIn />
  {:else}
    <div class="identity">
      <span class="avatar" aria-hidden="true">{initials(account.email, account.name)}</span>
      <div class="who">
        <h2>{account.name ?? account.email}</h2>
        {#if subtitle}<p>{subtitle}</p>{/if}
      </div>
    </div>
    {#if account.notice}<p class="note" data-tone={account.notice.warning ? "warning" : undefined}>
        {account.notice.text}
      </p>{/if}
    <AccountPlan />
    <AccountDevices />
    <SettingsGroup title={m.section_account()}>
      <SettingsRow title={m.account_sign_out_title()} description={m.account_sign_out_desc()}>
        <Button
          pending={busy === "sign_out"}
          disabled={!!busy && busy !== "sign_out"}
          onclick={() => void session.signOut()}>{m.account_sign_out()}</Button
        >
      </SettingsRow>
      <AccountDelete />
    </SettingsGroup>
    {#if signOutFault}<p class="note" data-tone="error" role="alert">{signOutFault}</p>{/if}
  {/if}
  <AccountServer />
{/if}

<style>
  .identity {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 0 30px;
  }

  .avatar {
    display: grid;
    flex: none;
    inline-size: 52px;
    block-size: 52px;
    border-radius: var(--radius-capsule);
    background: var(--color-fill-strong);
    color: var(--color-text);
    font-size: 19px;
    font-weight: 500;
    letter-spacing: 0.01em;
    place-items: center;
    box-shadow: inset 0 0 0 0.5px var(--color-border);
  }

  .who {
    min-inline-size: 0;
  }

  h2 {
    overflow: hidden;
    margin: 0;
    color: var(--color-text);
    font-size: 18px;
    font-weight: 500;
    letter-spacing: -0.02em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .who p {
    margin: 4px 0 0;
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .note {
    margin: 0 18px 24px;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 1.5;
  }

  .note[data-tone="error"] {
    color: var(--color-danger);
  }

  .note[data-tone="warning"] {
    color: var(--color-warning);
  }
</style>
