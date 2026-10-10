<script lang="ts">
  import Button from "$shared/ui/Button";
  import * as m from "$shared/i18n/messages";
  import HostGlyph from "../cards/HostGlyph.svelte";
  import AskReceipt from "./AskReceipt.svelte";
  import AskShell from "./AskShell.svelte";
  import { registrableSite, siteName, type SignInAsk } from "./asks";

  /** A page the agent can't go past until the person signs in; it carries on by itself after. */
  let {
    ask,
    placement = "canvas",
    busy = false,
    onopen,
  }: {
    ask: SignInAsk;
    placement?: "canvas" | "island";
    busy?: boolean;
    onopen?: () => void;
  } = $props();
  const title = $derived(
    ask.kind === "check"
      ? m.work_human_title_check({ host: siteName(ask.host) })
      : m.work_ask_sign_in_title({ host: siteName(ask.host) }),
  );
</script>

{#if ask.state === "open"}
  <AskShell {placement} {busy} label={title} where={registrableSite(ask.host)} {title}>
    {#snippet mark()}<HostGlyph host={ask.host} size={18} initial={false} />{/snippet}
    <p class="how">{m.work_ask_handoff_how()}</p>
    {#snippet actions()}
      {#if onopen}<Button variant="primary" disabled={busy} onclick={() => onopen?.()}
          >{ask.kind === "check" ? m.work_human_open() : m.work_ask_sign_in()}</Button
        >{/if}
    {/snippet}
  </AskShell>
{:else}
  <AskReceipt
    tone={ask.state === "done" ? "done" : ask.state === "failed" ? "failed" : "working"}
    status={ask.state === "done"
      ? m.work_ask_done()
      : ask.state === "failed"
        ? m.work_ask_not_done()
        : m.work_ask_continuing()}
    text={ask.state === "failed" ? title : siteName(ask.host)}
  />
{/if}

<style>
  .how {
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-body);
    line-height: 18px;
  }
</style>
