<script lang="ts">
  import { onMount } from "svelte";
  import { accountSession, initials, planName } from "$domain/account";
  import Menu, { type MenuEntry } from "$shared/ui/Menu";
  import { commands } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import { UserIcon } from "../../lib/icons";
  import * as m from "$shared/i18n/messages";

  /** The Work profile's name, shown while no account is signed in. */
  let { name = "" }: { name?: string } = $props();

  onMount(() => {
    void accountSession.retain();
    return () => accountSession.release();
  });

  const account = $derived(accountSession.account());
  const signedIn = $derived(account?.signed_in === true);
  const letters = $derived(signedIn ? initials(account?.email, account?.name) : "");
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
  const entries = $derived.by(() => {
    const list: MenuEntry[] = [];
    const heading = signedIn ? account?.email : name;
    if (heading) list.push({ kind: "heading", label: heading });
    if (summary) list.push({ kind: "heading", label: summary });
    if (account?.available)
      list.push({
        kind: "item",
        id: "settings.account",
        label: signedIn ? m.account_manage() : m.ai_cloud_sign_in(),
      });
    list.push({ kind: "item", id: "settings.ai", label: m.work_model_manage() });
    list.push({ kind: "separator" });
    list.push({ kind: "item", id: "browser.settings", label: m.account_settings(), hint: "⌘," });
    return list;
  });
</script>

<Menu
  label={m.account_menu()}
  {entries}
  side="bottom"
  align="end"
  triggerClass="account-trigger"
  contentClass="works-menu-solid"
  onselect={(id) => void commands.runCommand(id)}
>
  {#snippet trigger()}<span class="avatar" data-signed-in={signedIn} aria-hidden="true"
      >{#if letters}{letters}{:else}<Icon icon={UserIcon} size={15} strokeWidth={1.7} />{/if}</span
    >{/snippet}
</Menu>

<style>
  :global(.works-menu-solid) {
    background: var(--color-float);
    backdrop-filter: none;
  }

  :global(.account-trigger) {
    display: grid;
    inline-size: 30px;
    block-size: 30px;
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
    inline-size: 24px;
    block-size: 24px;
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
