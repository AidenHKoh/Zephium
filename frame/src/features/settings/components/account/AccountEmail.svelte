<script lang="ts">
  import type { Component } from "svelte";
  import { accountSession as session } from "$domain/account";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import SettingsRow from "$shared/ui/SettingsRow";

  const account = $derived(session.account());
  const busy = $derived(session.pending());
  let open = $state(false);
  let opener = $state.raw<HTMLElement | null>(null);
  /** The change flow loads only when someone starts it. */
  let Flow = $state.raw<Component<{ onclose: () => void }> | null>(null);

  async function start() {
    opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    session.clearFault();
    Flow ??= (await import("./AccountEmailFlow.svelte")).default;
    open = true;
  }

  function close() {
    open = false;
    if (opener?.isConnected) opener.focus();
  }
</script>

<SettingsRow title={m.account_email_title()} description={account?.email ?? undefined}>
  {#if !open}<Button size="compact" disabled={!!busy} onclick={() => void start()}
      >{m.account_email_change()}</Button
    >{/if}
</SettingsRow>

{#if Flow && open}<Flow onclose={close} />{/if}
