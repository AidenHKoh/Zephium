<script lang="ts">
  import { onMount } from "svelte";
  import { UserIcon } from "@hugeicons/core-free-icons";
  import { accountSession, initials, planName } from "$domain/account";
  import { commands } from "$shared/ipc/bindings";
  import * as m from "$shared/i18n/messages";
  import Disclosure from "$shared/ui/Disclosure";
  import Icon from "$shared/ui/Icon";

  /** The profile's name, shown while no account is signed in. */
  let { name = "" }: { name?: string } = $props();

  onMount(() => {
    void accountSession.retain();
    return () => accountSession.release();
  });

  const account = $derived(accountSession.account());
  const signedIn = $derived(account?.signed_in === true);
  const letters = $derived(signedIn ? initials(account?.email, account?.name) : "");
  const heading = $derived(signedIn ? account?.email : name);
  const summary = $derived.by(() => {
    if (!signedIn || !account) return null;
    const plan = planName(account.plan);
    const used = account.usage?.used_percent;
    const parts = [
      plan ? m.account_plan_line({ plan }) : null,
      used != null ? m.ai_cloud_used({ percent: used }) : null,
    ];
    return parts.filter(Boolean).join(" · ") || null;
  });

  const run = (id: string) => void commands.runCommand(id).catch(() => {});
</script>

<!--
  The rail's own lightweight menu rather than the page-wide one: this loads
  with Work into the chrome, which pays for every dependency at launch.
-->
<Disclosure
  label={m.account_menu()}
  menu
  triggerClass="account-trigger"
  panelClass="account-panel"
>
  {#snippet trigger()}<span class="avatar" data-signed-in={signedIn} aria-hidden="true"
      >{#if letters}{letters}{:else}<Icon icon={UserIcon} size={15} strokeWidth={1.7} />{/if}</span
    >{/snippet}
  {#if heading}<div class="heading" role="presentation">{heading}</div>{/if}
  {#if summary}<div class="heading summary" role="presentation">{summary}</div>{/if}
  {#if account?.available}<button
      type="button"
      role="menuitem"
      class="ui-menu-item item"
      onclick={() => run("settings.account")}
      >{signedIn ? m.account_manage() : m.ai_cloud_sign_in()}</button
    >{/if}
  <button type="button" role="menuitem" class="ui-menu-item item" onclick={() => run("settings.ai")}
    >{m.work_model_manage()}</button
  >
  <div class="separator" role="separator"></div>
  <button
    type="button"
    role="menuitem"
    class="ui-menu-item item"
    onclick={() => run("browser.settings")}
    >{m.account_settings()}<span class="hint" aria-hidden="true">⌘,</span></button
  >
</Disclosure>

<style>
  /* Out of the rail, beside it, its foot level with the trigger's. */
  :global(.account-panel.ui-disclosure-panel) {
    top: auto;
    bottom: 0;
    inset-inline-start: calc(100% + 10px);
    min-width: 220px;
    max-width: 300px;
    transform-origin: bottom left;
    translate: -4px 0;
  }

  :global(.account-panel.ui-disclosure-panel[data-open="true"]) {
    translate: 0 0;
  }

  .heading {
    overflow: hidden;
    padding: 8px var(--menu-item-inset) 2px;
    color: var(--color-text);
    font-size: var(--text-label);
    font-weight: 550;
    line-height: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .summary {
    padding-block: 0 6px;
    color: var(--color-faint);
    font-weight: 450;
  }

  .item {
    inline-size: 100%;
    border: 0;
    background: transparent;
    font: inherit;
    text-align: start;
  }

  .item:hover,
  .item:focus-visible {
    background: var(--row-active);
  }

  .item:active {
    background: var(--row-pressed);
  }

  .hint {
    margin-inline-start: auto;
    padding-inline-start: 28px;
    color: var(--color-faint);
    font-size: var(--text-label);
  }

  .separator {
    flex: none;
    height: 1px;
    margin: 5px calc(var(--menu-pad) * -1);
    background: var(--color-menu-separator);
  }

  :global(.account-trigger) {
    display: grid;
    inline-size: 36px;
    block-size: 36px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-capsule);
    background: transparent;
    place-items: center;
    cursor: default;
    pointer-events: auto;
    transition: background-color var(--motion-fast) var(--ease-out);
  }

  :global(.account-trigger:hover),
  :global(.account-trigger[data-state="open"]) {
    background: var(--color-fill-hover);
  }

  :global(.account-trigger:focus-visible) {
    outline: 2px solid var(--color-ring);
    outline-offset: 1px;
  }

  .avatar {
    display: grid;
    inline-size: 26px;
    block-size: 26px;
    border-radius: var(--radius-capsule);
    background: var(--color-fill-strong);
    color: var(--color-label-secondary);
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.02em;
    place-items: center;
    box-shadow: inset 0 0 0 0.5px var(--color-border);
  }

  .avatar[data-signed-in="true"] {
    color: var(--color-text);
  }
</style>
