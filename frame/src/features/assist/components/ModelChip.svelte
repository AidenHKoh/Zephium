<script lang="ts">
  import { onMount } from "svelte";
  import * as m from "$shared/i18n/messages";
  import {
    ModelsSession,
    entryOf,
    locked,
    pickerGroups,
    providerName,
    shortName,
  } from "$domain/ai";
  import { commands } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import { ArrowDown01Icon, Tick02Icon } from "@hugeicons/core-free-icons";

  let { profile }: { profile: string } = $props();
  const uid = $props.id();

  let session = $state.raw<ModelsSession | null>(null);
  $effect(() => {
    const owner = new ModelsSession(profile);
    session = owner;
    void owner.start();
    return () => owner.dispose();
  });

  const models = $derived(session?.models ?? null);
  const current = $derived(entryOf(models, models?.effective.page));
  const chosen = $derived(models?.chosen.page ?? null);
  const groups = $derived(pickerGroups(models, "page", session?.endpoint ?? []).ready);
  let open = $state(false);
  let root = $state<HTMLElement>();
  let list = $state<HTMLElement>();

  function choose(id: string | null) {
    open = false;
    void session?.choose("page", id);
  }

  onMount(() => {
    const away = (event: PointerEvent) => {
      if (open && root && !root.contains(event.target as Node)) open = false;
    };
    document.addEventListener("pointerdown", away, true);
    return () => document.removeEventListener("pointerdown", away, true);
  });

  $effect(() => {
    if (open)
      queueMicrotask(() => list?.querySelector<HTMLElement>("[aria-selected=true]")?.focus());
  });

  function onkeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      // Closes the list, not the panel behind it.
      event.stopPropagation();
      open = false;
      root?.querySelector<HTMLElement>("button")?.focus();
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const options = [
      ...(list?.querySelectorAll<HTMLElement>("[role=option]:not(:disabled)") ?? []),
    ];
    const at = options.indexOf(document.activeElement as HTMLElement);
    const next = event.key === "ArrowDown" ? at + 1 : at - 1;
    options[(next + options.length) % options.length]?.focus();
  }
</script>

<div class="chip-root" bind:this={root} {onkeydown} role="presentation">
  <button
    type="button"
    class="chip"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={open ? `${uid}-list` : undefined}
    disabled={!models}
    title={m.assist_model_label()}
    onclick={() => (open = !open)}
  >
    {#if current}<span class="name">{shortName(current)}</span>{:else}<span class="name"
        >{m.assist_model_none()}</span
      >{/if}
    <Icon icon={ArrowDown01Icon} size={12} />
  </button>
  {#if open && models}
    <div
      class="menu assist-pop"
      id="{uid}-list"
      role="listbox"
      aria-label={m.assist_model_label()}
      bind:this={list}
    >
      <button
        type="button"
        role="option"
        class="option"
        aria-selected={chosen === null}
        onclick={() => choose(null)}
      >
        <span class="o-name"
          >{current ? m.ai_role_auto({ model: shortName(current) }) : m.ai_role_auto_none()}</span
        >
        {#if chosen === null}<Icon icon={Tick02Icon} size={13} strokeWidth={2.2} />{/if}
      </button>
      {#each groups as group (group.provider)}
        <div class="group" role="presentation">{providerName(group.provider)}</div>
        {#each group.entries as entry (entry.id)}
          <button
            type="button"
            role="option"
            class="option"
            aria-selected={chosen === entry.id}
            disabled={locked(models, entry.id)}
            onclick={() => choose(entry.id)}
          >
            <span class="o-name">{entry.display_name}</span>
            {#if chosen === entry.id}<Icon icon={Tick02Icon} size={13} strokeWidth={2.2} />{/if}
          </button>
        {/each}
      {/each}
      <div class="rule" role="presentation"></div>
      <button
        type="button"
        class="option more"
        onclick={() => {
          open = false;
          void commands.runCommand("settings.ai").catch(() => {});
        }}><span class="o-name">{m.work_model_manage()}</span></button
      >
    </div>
  {/if}
</div>

<style>
  .chip-root {
    position: relative;
    min-inline-size: 0;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-inline-size: 100%;
    block-size: 24px;
    padding: 0 7px 0 8px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: transparent;
    color: var(--color-muted);
    font: inherit;
    font-size: var(--text-label);
    font-weight: 500;
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth);
  }

  .chip:focus-visible {
    outline: 2px solid var(--color-ring);
  }

  .chip:hover:not(:disabled),
  .chip[aria-expanded="true"] {
    background: var(--color-fill-hover);
    color: var(--color-text);
  }

  .name {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .menu {
    position: absolute;
    inset-block-end: calc(100% + 8px);
    inset-inline-start: -4px;
    z-index: 2;
    display: flex;
    flex-direction: column;
    inline-size: 248px;
    max-block-size: 280px;
    box-sizing: border-box;
    overflow: auto;
    padding: 5px;
    border-radius: var(--radius-row);
    background: var(--color-float);
    box-shadow: var(--shadow-popover);
  }

  .assist-pop {
    animation: pop 140ms var(--ease-out) both;
    transform-origin: bottom left;
  }

  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(4px) scale(0.98);
    }
  }

  .option {
    display: flex;
    align-items: center;
    gap: 8px;
    min-block-size: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--radius-inset);
    background: transparent;
    color: var(--color-label-secondary);
    font: inherit;
    font-size: var(--text-body);
    text-align: start;
    cursor: default;
  }

  .option:disabled {
    opacity: 0.45;
  }

  .option:focus-visible,
  .option:hover:not(:disabled) {
    background: var(--color-fill-hover);
    color: var(--color-text);
    outline: none;
  }

  .o-name {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .more {
    color: var(--color-muted);
  }

  .group {
    padding: 8px 8px 3px;
    color: var(--color-faint);
    font-size: var(--text-caption);
    font-weight: 600;
  }

  .rule {
    flex: none;
    block-size: 1px;
    margin: 4px 6px;
    background: var(--color-border);
  }

  @media (prefers-reduced-motion: reduce) {
    .assist-pop {
      animation: none;
    }
  }
</style>
