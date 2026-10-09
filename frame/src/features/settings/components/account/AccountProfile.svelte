<script lang="ts">
  import { accountSession as session } from "$domain/account";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Field from "$shared/ui/Field";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import { explainAccountFault } from "../../lib/account";
  import AccountEmail from "./AccountEmail.svelte";

  const account = $derived(session.account());
  const busy = $derived(session.pending());
  const fault = $derived(session.fault());
  let editing = $state(false);
  let name = $state("");

  const faultText = $derived(
    fault?.action === "name" ? explainAccountFault(fault.fault, fault.retryAfter) : null,
  );

  async function save() {
    const trimmed = name.trim();
    if (await session.setName(trimmed ? trimmed : null)) editing = false;
  }
</script>

<SettingsGroup title={m.account_profile_group()}>
  <SettingsRow title={m.account_name_title()} description={account?.name ?? m.account_name_none()}>
    {#if !editing}<Button
        size="compact"
        disabled={!!busy}
        onclick={() => {
          session.clearFault();
          name = account?.name ?? "";
          editing = true;
        }}>{m.account_name_edit()}</Button
      >{/if}
  </SettingsRow>
  {#if editing}<form
      class="editor"
      onsubmit={(event) => {
        event.preventDefault();
        void save();
      }}
    >
      <Field
        label={m.account_name_title()}
        labelHidden
        autocomplete="name"
        maxlength={100}
        bind:value={name}
        onkeydown={(event) => {
          if (event.key === "Escape") editing = false;
        }}
      />
      <Button size="compact" variant="ghost" onclick={() => (editing = false)}
        >{m.action_cancel()}</Button
      >
      <Button size="compact" variant="primary" type="submit" pending={busy === "name"}
        >{m.account_name_save()}</Button
      >
    </form>{/if}
  {#if faultText}<p class="error" role="alert">{faultText}</p>{/if}
  <AccountEmail />
</SettingsGroup>

<style>
  .editor {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 18px 14px;
  }

  .editor :global(.ui-field) {
    flex: 1;
  }

  .error {
    margin: 0;
    padding: 0 18px 14px;
    color: var(--color-danger);
    font-size: var(--text-label);
  }
</style>
