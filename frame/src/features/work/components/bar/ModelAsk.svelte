<script lang="ts">
  import type { WorkModelProvider } from "$shared/ipc/bindings";
  import ProviderMark from "$shared/ui/ProviderMark";
  import ZephiumTile from "$shared/ui/ZephiumTile";
  import Icon from "$shared/ui/Icon";
  import "$shared/ui/Shine/shine.css";
  import { Key01Icon, UserIcon } from "../../lib/icons";
  import * as m from "$shared/i18n/messages";
  import type { Standing } from "./ModelRail.svelte";

  let {
    provider,
    name,
    standing,
    onact,
  }: {
    provider: WorkModelProvider;
    name: string;
    standing: Exclude<Standing, "ready">;
    onact: () => void;
  } = $props();

  const body = $derived(
    standing === "sign_in"
      ? m.model_hosted()
      : standing === "refused"
        ? m.model_key_refused({ provider: name })
        : standing === "server"
          ? m.model_server_needed()
          : m.model_key_needed({ provider: name }),
  );
  const action = $derived(
    standing === "sign_in"
      ? m.work_model_sign_in()
      : standing === "server"
        ? m.model_add_server()
        : m.model_add_key(),
  );
</script>

<div class="ask">
  <span class="tile ui-shine" aria-hidden="true"
    >{#if provider === "cloud"}<ZephiumTile size={30} />{:else}<ProviderMark
        {provider}
        size={26}
      />{/if}</span
  >
  <p class="body" data-refused={standing === "refused" ? "" : undefined}>{body}</p>
  <button type="button" class="act ui-shine ui-shine-press" onclick={onact}
    ><Icon
      icon={standing === "sign_in" ? UserIcon : Key01Icon}
      size={14}
      strokeWidth={1.8}
    />{action}</button
  >
</div>

<style>
  .ask {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 16px 28px 28px;
    text-align: center;
  }

  .tile {
    --shine-radius: var(--radius-control-large);

    display: grid;
    inline-size: 56px;
    block-size: 56px;
    place-items: center;
  }

  .body {
    max-inline-size: 250px;
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-body);
    line-height: 19px;
    text-wrap: balance;
  }

  .body[data-refused] {
    color: var(--color-danger);
  }

  .act {
    --shine-radius: var(--radius-capsule);

    display: inline-flex;
    align-items: center;
    gap: 7px;
    block-size: 32px;
    padding-inline: 14px 16px;
    border: 0;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-label);
    font-weight: 550;
  }
</style>
