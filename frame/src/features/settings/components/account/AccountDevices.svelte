<script lang="ts">
  import { accountSession as session, shortDate } from "$domain/account";
  import type { AccountFaultV1, AccountSessionV1 } from "$shared/ipc/bindings";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import { explainAccountFault } from "../../lib/account";

  let shown = $state(false);
  let loading = $state(false);
  let devices = $state.raw<AccountSessionV1[]>([]);
  let failure = $state<AccountFaultV1 | null>(null);
  let revoking = $state<string | null>(null);

  async function load() {
    shown = true;
    loading = true;
    const result = await session.sessions();
    loading = false;
    if (typeof result === "string") {
      failure = result;
      return;
    }
    failure = null;
    devices = [...result].sort((a, b) => Number(b.current) - Number(a.current));
  }

  async function revoke(id: string) {
    revoking = id;
    const ok = await session.revoke(id);
    revoking = null;
    if (ok) devices = devices.filter((device) => device.id !== id);
  }

  function since(device: AccountSessionV1) {
    const date = shortDate(device.created_at);
    return date ? m.account_device_since({ date }) : undefined;
  }
</script>

<SettingsGroup title={m.account_devices_group()}>
  {#if !shown}<SettingsRow
      title={m.account_devices_row()}
      description={m.account_devices_row_desc()}
      ><Button onclick={() => void load()}>{m.account_devices_show()}</Button></SettingsRow
    >
  {:else if loading && !devices.length}<p class="empty">{m.account_usage_loading()}</p>
  {:else if failure}<p class="empty" role="alert">{explainAccountFault(failure, null)}</p>
  {:else}
    {#each devices as device (device.id)}
      <SettingsRow
        title={device.current
          ? m.account_device_this()
          : (device.device ?? m.account_device_unknown())}
        description={device.current ? (device.device ?? since(device)) : since(device)}
      >
        {#if !device.current}<Button
            size="compact"
            variant="ghost"
            pending={revoking === device.id}
            disabled={!!revoking}
            onclick={() => void revoke(device.id)}>{m.account_device_sign_out()}</Button
          >{/if}
      </SettingsRow>
    {:else}<p class="empty">{m.account_devices_empty()}</p>
    {/each}
  {/if}
</SettingsGroup>

<style>
  .empty {
    margin: 0;
    padding: 14px 18px;
    color: var(--color-muted);
    font-size: var(--text-label);
  }
</style>
