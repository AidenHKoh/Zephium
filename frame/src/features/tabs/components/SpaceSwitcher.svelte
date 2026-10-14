<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { Add01Icon, PencilEdit02Icon, Tick02Icon } from "@hugeicons/core-free-icons";
  import type { SpaceView } from "$shared/ipc/bindings";
  import { tabs } from "$domain/tabs";
  import { keymap } from "$domain/keymap";
  import { acceleratorKeys } from "$shared/lib/accelerator";
  import { IS_MAC } from "$shared/platform";
  import Icon from "$shared/ui/Icon";
  import SpaceGlyph from "./SpaceGlyph.svelte";

  let {
    spaces,
    active,
    onchoose,
    making = false,
    failed = false,
    onedit,
    onnew,
  }: {
    spaces: SpaceView[];
    active: string | null;
    /** A new space was asked for and has not appeared yet. */
    making?: boolean;
    failed?: boolean;
    onchoose: (id: string) => void;
    onedit: (space: SpaceView) => void;
    onnew: () => void;
  } = $props();

  function keysFor(index: number) {
    const accelerator = keymap
      .all()
      .find((entry) => entry.id === `space.select.${index + 1}`)?.accelerator;
    return accelerator ? acceleratorKeys(accelerator, IS_MAC).join(IS_MAC ? "" : "+") : null;
  }

  // Rows reorder by dragging them, or with Option and the arrows.
  let list = $state<HTMLElement>();
  let held = $state<{ id: string; from: number; y: number; to: number } | null>(null);
  let moved = $state(false);

  function down(event: PointerEvent, id: string, from: number) {
    if (event.button !== 0) return;
    held = { id, from, y: event.clientY, to: from };
    moved = false;
  }

  function move(event: PointerEvent) {
    if (!held || !list) return;
    if (!moved && Math.abs(event.clientY - held.y) < 5) return;
    if (!moved) (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    moved = true;
    const rows = [...list.querySelectorAll<HTMLElement>("[data-space-row]")];
    const to = rows.findIndex((row) => {
      const rect = row.getBoundingClientRect();
      return event.clientY < rect.top + rect.height / 2;
    });
    held = { ...held, to: to === -1 ? rows.length : to };
  }

  function up() {
    if (!held) return;
    const { id, from, to } = held;
    held = null;
    if (!moved || to === from || to === from + 1) return;
    void tabs.reorderSpace(id, spaces[to]?.id ?? null);
  }

  function choose(id: string) {
    if (moved) {
      moved = false;
      return;
    }
    onchoose(id);
  }

  function keydown(event: KeyboardEvent, index: number) {
    if (!event.altKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;
    event.preventDefault();
    event.stopPropagation();
    const id = spaces[index]!.id;
    if (event.key === "ArrowUp" && index > 0) void tabs.reorderSpace(id, spaces[index - 1]!.id);
    if (event.key === "ArrowDown" && index < spaces.length - 1)
      void tabs.reorderSpace(id, spaces[index + 2]?.id ?? null);
  }
</script>

<div class="switcher" role="menu" aria-label={m.space_switcher()} bind:this={list}>
  {#each spaces as space, index (space.id)}
    {@const current = space.id === active}
    {@const keys = keysFor(index)}
    <div
      class="switch-row"
      data-space-row
      data-held={held?.id === space.id && moved ? "" : undefined}
      data-drop-before={held && moved && held.to === index && held.from !== index ? "" : undefined}
      data-drop-after={held && moved && held.to === spaces.length && index === spaces.length - 1
        ? ""
        : undefined}
    >
      <button
        type="button"
        role="menuitemradio"
        class="ui-menu-item switch-pick"
        aria-checked={current}
        onpointerdown={(event) => down(event, space.id, index)}
        onpointermove={move}
        onpointerup={up}
        onpointercancel={() => (held = null)}
        onkeydown={(event) => keydown(event, index)}
        onclick={() => choose(space.id)}
      >
        <span class="switch-glyph"
          ><SpaceGlyph
            icon={space.icon}
            color={space.color}
            name={space.name}
            fallback="dot"
          /></span
        >
        <span class="switch-name">{space.name}</span>
        {#if keys}<span class="switch-keys">{keys}</span>{/if}
        <span class="switch-mark"
          >{#if current}<Icon icon={Tick02Icon} size={14} strokeWidth={2} />{/if}</span
        >
      </button>
      <button
        type="button"
        class="switch-edit"
        title={m.space_edit_label({ name: space.name })}
        aria-label={m.space_edit_label({ name: space.name })}
        onclick={() => onedit(space)}><Icon icon={PencilEdit02Icon} size={14} /></button
      >
    </div>
  {/each}
  <div class="switch-rule" role="separator"></div>
  <button
    type="button"
    role="menuitem"
    class="ui-menu-item switch-new"
    aria-disabled={making || undefined}
    onclick={() => !making && onnew()}
  >
    <span class="switch-glyph"><Icon icon={Add01Icon} size={15} strokeWidth={1.8} /></span>
    <span>{m.space_new()}</span>
  </button>
  {#if failed}<p class="switch-error" role="alert">{m.space_new_failed()}</p>{/if}
</div>

<style>
  .switcher {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .switch-row {
    position: relative;
    display: flex;
    align-items: center;
  }

  .switch-pick,
  .switch-new {
    flex: 1;
    min-width: 0;
    width: 100%;
    min-height: 32px;
    padding-block: 4px;
    border: 0;
    background: transparent;
    font: inherit;
    font-size: var(--text-body);
    text-align: start;
  }

  .switch-new {
    color: var(--color-muted);
  }

  .switch-new[aria-disabled] {
    opacity: 0.55;
  }

  .switch-pick:hover,
  .switch-pick:focus-visible,
  .switch-new:hover,
  .switch-new:focus-visible {
    outline: none;
    background: var(--row-active);
  }

  .switch-pick:active,
  .switch-new:active {
    background: var(--row-pressed);
  }

  /* One column for every mark, so names line up whatever a space wears. */
  .switch-glyph {
    display: grid;
    flex: none;
    width: 18px;
    place-items: center;
  }

  .switch-new .switch-glyph {
    color: var(--color-faint);
  }

  .switch-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .switch-pick[aria-checked="true"] .switch-name {
    font-weight: 600;
  }

  .switch-keys {
    color: var(--color-faint);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
    transition: opacity var(--motion-instant) var(--ease-out);
  }

  .switch-mark {
    display: grid;
    flex: none;
    width: 14px;
    color: var(--color-text);
    place-items: center;
  }

  /* Edit waits at the row's end and stands in for its key on approach. */
  .switch-edit {
    position: absolute;
    display: grid;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--color-muted);
    opacity: 0;
    place-items: center;
    inset-inline-end: 26px;
    pointer-events: none;
    transition:
      opacity var(--motion-fast) var(--ease-out),
      background-color var(--motion-instant) var(--ease-out),
      color var(--motion-instant) var(--ease-out);
  }

  .switch-edit:hover,
  .switch-edit:focus-visible {
    outline: none;
    background: var(--row-pressed);
    color: var(--color-text);
    opacity: 1;
    pointer-events: auto;
  }

  .switch-row:hover .switch-edit {
    opacity: 1;
    pointer-events: auto;
  }

  .switch-row:hover .switch-keys {
    opacity: 0;
  }

  .switch-row[data-held] {
    opacity: 0.55;
  }

  .switch-row[data-drop-before]::before,
  .switch-row[data-drop-after]::after {
    position: absolute;
    height: 2px;
    border-radius: var(--radius-capsule);
    background: var(--color-text);
    content: "";
    inset-inline: 10px;
  }

  .switch-row[data-drop-before]::before {
    inset-block-start: -1px;
  }

  .switch-row[data-drop-after]::after {
    inset-block-end: -1px;
  }

  .switch-rule {
    height: 1px;
    margin: 4px var(--menu-item-inset);
    background: var(--color-border);
  }

  .switch-error {
    padding: 2px var(--menu-item-inset) 4px;
    color: var(--color-danger);
    font-size: var(--text-label);
    line-height: 16px;
  }

  @media (prefers-reduced-motion: reduce) {
    .switch-edit {
      transition: none;
    }
  }
</style>
