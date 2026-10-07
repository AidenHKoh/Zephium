<script lang="ts">
  import { accountSession as session } from "$domain/account";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Field from "$shared/ui/Field";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import { explainAccountFault } from "../../lib/account";

  const account = $derived(session.account());
  const busy = $derived(session.pending());
  const fault = $derived(session.fault());
  let editing = $state(false);
  let address = $state("");

  const host = $derived.by(() => {
    try {
      return new URL(account?.server ?? "").host;
    } catch {
      return account?.server ?? "";
    }
  });
  const faultText = $derived(
    fault?.action === "server" ? explainAccountFault(fault.fault, fault.retryAfter) : null,
  );

  async function save(server: string | null) {
    if (await session.setServer(server)) editing = false;
  }
</script>

<SettingsGroup title={m.account_server_group()} description={m.account_server_note()}>
  <SettingsRow
    title={m.account_server_title()}
    description={account?.custom_server ? host : m.account_server_default({ server: host })}
  >
    {#if account?.custom_server}<Button
        size="compact"
        variant="ghost"
        pending={busy === "server"}
        disabled={!!busy}
        onclick={() => void save(null)}>{m.account_server_reset()}</Button
      >{/if}
    {#if !editing}<Button
        size="compact"
        disabled={!!busy}
        onclick={() => {
          address = account?.custom_server ? (account.server ?? "") : "";
          editing = true;
        }}>{m.account_server_change()}</Button
      >{/if}
  </SettingsRow>
  {#if editing}<form
      class="editor"
      onsubmit={(event) => {
        event.preventDefault();
        void save(address.trim());
      }}
    >
      <Field
        label={m.account_server_field()}
        labelHidden
        type="url"
        placeholder="https://accounts.example.com"
        autocomplete="off"
        spellcheck="false"
        bind:value={address}
        onkeydown={(event) => {
          if (event.key === "Escape") editing = false;
        }}
      />
      <Button size="compact" variant="ghost" onclick={() => (editing = false)}
        >{m.action_cancel()}</Button
      >
      <Button
        size="compact"
        variant="primary"
        type="submit"
        pending={busy === "server"}
        disabled={!address.trim()}>{m.account_server_save()}</Button
      >
    </form>{/if}
  {#if faultText}<p class="error" role="alert">{faultText}</p>{/if}
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
