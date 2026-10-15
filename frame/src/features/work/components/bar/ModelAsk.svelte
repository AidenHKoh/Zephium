<script lang="ts">
  import type { WorkModelProvider } from "$shared/ipc/bindings";
  import ProviderMark from "$shared/ui/ProviderMark";
  import ZephiumTile from "$shared/ui/ZephiumTile";
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
  <span class="tile" aria-hidden="true"
    >{#if provider === "cloud"}<ZephiumTile size={30} />{:else}<ProviderMark
        {provider}
        size={26}
      />{/if}</span
  >
  <p class="body" data-refused={standing === "refused" ? "" : undefined}>{body}</p>
  <button type="button" class="act" onclick={onact}>{action}</button>
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
    display: grid;
    inline-size: 52px;
    block-size: 52px;
    place-items: center;
    border-radius: var(--radius-control-large);
    background: var(--row-active);
    box-shadow: var(--shine-top);
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
    block-size: 30px;
    padding-inline: 16px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--row-active);
    box-shadow: var(--shine-top);
    color: var(--color-text);
    cursor: default;
    font: inherit;
    font-size: var(--text-label);
    font-weight: 550;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      scale var(--motion-base) var(--ease-spring);
  }

  .act:hover {
    background: var(--row-pressed);
  }

  .act:active {
    scale: 0.96;
  }

  .act:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: 2px;
  }

  @media (prefers-reduced-motion: reduce) {
    .act {
      transition: none;
    }
  }
</style>
