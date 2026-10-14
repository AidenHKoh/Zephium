<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { ArrowLeft01Icon, Tick02Icon } from "@hugeicons/core-free-icons";
  import type { SpaceIconView, SpaceThemeView, SpaceView } from "$shared/ipc/bindings";
  import { tabs } from "$domain/tabs";
  import { palette, theme as appearance } from "$domain/appearance";
  import { preferences } from "$domain/preferences";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import { onMount, untrack } from "svelte";
  import SpaceGlyph from "./SpaceGlyph.svelte";
  import { SPACE_COLORS, SPACE_SYMBOLS, emojiOf, freshLook } from "../lib/space-look";

  let {
    space,
    spaces,
    onclose,
  }: {
    /** The space being edited, or null for a new one. */
    space: SpaceView | null;
    spaces: SpaceView[];
    /** `done` when the space was saved or deleted. */
    onclose: (done: boolean) => void;
  } = $props();

  // The editor starts from the space as it was opened; later projections do
  // not overwrite what is being typed.
  const start = untrack(() => space ?? { ...freshLook(spaces), name: "" });
  let name = $state(untrack(() => space?.name ?? ""));
  let icon = $state<SpaceIconView | null>(start.icon);
  let color = $state(start.color);
  let chosen = $state<SpaceThemeView | null>(start.theme);
  let emoji = $state(start.icon?.kind === "emoji" ? start.icon.text : "");
  let saving = $state(false);
  let failed = $state(false);
  let confirming = $state(false);
  let field = $state<HTMLInputElement>();

  const appTheme = $derived(
    palette.resolveTheme(preferences.value("ui.theme"), preferences.value("ui.theme-custom")),
  );
  // The app's own custom theme can be worn by a space too.
  const customSeeds = $derived(
    chosen?.kind === "custom"
      ? chosen.seeds
      : appTheme.custom
        ? preferences.value("ui.theme-custom")
        : null,
  );
  const mode = () =>
    document.documentElement.dataset.theme === "light" ? ("light" as const) : ("dark" as const);
  const swatch = (spec: palette.ThemeSpec) => spec[mode()];
  const keyOf = (value: SpaceThemeView | null) =>
    value === null ? "" : value.kind === "preset" ? `preset:${value.id}` : `custom:${value.seeds}`;
  const same = (a: SpaceThemeView | null, b: SpaceThemeView | null) => keyOf(a) === keyOf(b);

  // Choosing a theme shows it on the whole window at once; leaving puts the
  // window back the way it was.
  $effect(() => {
    const next = chosen;
    if (next === null) appearance.previewTheme(appTheme.spec);
    else if (next.kind === "preset")
      appearance.previewTheme(palette.forkOf(palette.preset(next.id) ?? palette.presets[0]!));
    else appearance.previewTheme(palette.parseTheme(next.seeds));
  });
  onMount(() => {
    if (!space) field?.focus();
    return () => appearance.previewTheme(null);
  });

  function typeEmoji(text: string) {
    emoji = text;
    const found = emojiOf(text);
    if (found) icon = { kind: "emoji", text: found };
  }

  async function save(event?: SubmitEvent) {
    event?.preventDefault();
    if (saving) return;
    const trimmed = name.trim();
    saving = true;
    failed = false;
    const look = { icon, color, theme: chosen };
    const result = space
      ? await tabs.updateSpace(space.id, trimmed || space.name, look)
      : await tabs.createSpace(trimmed, look);
    saving = false;
    if (result.outcome === "failed" || result.outcome === "rejected") failed = true;
    else onclose(true);
  }

  async function remove() {
    if (!space || saving) return;
    saving = true;
    const result = await tabs.deleteSpace(space.id);
    saving = false;
    if (result.outcome === "failed" || result.outcome === "rejected") failed = true;
    else onclose(true);
  }

  const closing = (count: number) =>
    count === 0
      ? m.space_delete_body_none()
      : count === 1
        ? m.space_delete_body_one()
        : m.space_delete_body_many({ count: String(count) });
</script>

<form class="editor" onsubmit={save} aria-label={space ? m.space_edit_title() : m.space_new()}>
  <header class="editor-head">
    <button
      type="button"
      class="editor-back"
      aria-label={m.space_back()}
      onclick={() => onclose(false)}><Icon icon={ArrowLeft01Icon} size={14} /></button
    >
    <span class="editor-title">{space ? m.space_edit_title() : m.space_new()}</span>
  </header>

  <label class="editor-name">
    <SpaceGlyph {icon} {color} name={name || space?.name || "?"} size={30} />
    <input
      bind:this={field}
      bind:value={name}
      maxlength={48}
      placeholder={space?.name ?? m.space_name_placeholder()}
      aria-label={m.space_name()}
      spellcheck="false"
      autocomplete="off"
    />
  </label>

  <fieldset class="editor-group">
    <legend>{m.space_icon()}</legend>
    <div class="symbols" role="radiogroup" aria-label={m.space_icon()}>
      {#each SPACE_SYMBOLS as symbol, index (index)}
        {@const on = icon?.kind === "symbol" && icon.index === index}
        <button
          type="button"
          role="radio"
          class="symbol"
          aria-checked={on}
          aria-label={m.space_symbol({ index: String(index + 1) })}
          onclick={() => (icon = { kind: "symbol", index })}
          ><Icon icon={symbol} size={15} strokeWidth={1.8} /></button
        >
      {/each}
    </div>
    <input
      class="emoji-field"
      value={emoji}
      oninput={(event) => typeEmoji(event.currentTarget.value)}
      placeholder={m.space_emoji_placeholder()}
      aria-label={m.space_emoji_placeholder()}
      data-on={icon?.kind === "emoji" || undefined}
      maxlength={16}
    />
  </fieldset>

  <fieldset class="editor-group">
    <legend>{m.space_color()}</legend>
    <div class="colors" role="radiogroup" aria-label={m.space_color()}>
      {#each Array.from({ length: SPACE_COLORS }, (_, hue) => hue) as hue (hue)}
        <button
          type="button"
          role="radio"
          class="color"
          data-color={hue}
          aria-checked={color === hue}
          aria-label={m.space_color_index({ index: String(hue + 1) })}
          onclick={() => (color = hue)}
        ></button>
      {/each}
    </div>
  </fieldset>

  <fieldset class="editor-group">
    <legend>{m.space_theme()}</legend>
    <div class="themes" role="radiogroup" aria-label={m.space_theme()}>
      {#snippet chip(label: string, value: SpaceThemeView | null, seeds: palette.Seeds)}
        <button
          type="button"
          role="radio"
          class="theme"
          aria-checked={same(chosen, value)}
          title={label}
          aria-label={label}
          style:--chip-bg={seeds.bg}
          style:--chip-fg={seeds.fg}
          style:--chip-accent={seeds.accent}
          onclick={() => (chosen = value)}
        >
          <span class="theme-swatch"><i></i></span>
          {#if same(chosen, value)}<span class="theme-tick"
              ><Icon icon={Tick02Icon} size={10} strokeWidth={2.6} /></span
            >{/if}
        </button>
      {/snippet}
      {@render chip(m.space_theme_app(), null, swatch(appTheme.spec))}
      {#each palette.presets as entry (entry.id)}
        {@render chip(entry.name, { kind: "preset", id: entry.id }, entry[mode()])}
      {/each}
      {#if customSeeds}{@const spec = palette.parseTheme(customSeeds)}{#if spec}{@render chip(
            m.space_theme_custom(),
            { kind: "custom", seeds: customSeeds },
            swatch(spec),
          )}{/if}{/if}
    </div>
    <p class="theme-note">
      {chosen === null
        ? m.space_theme_follows()
        : m.space_theme_own({
            name:
              chosen.kind === "preset"
                ? (palette.preset(chosen.id)?.name ?? chosen.id)
                : m.space_theme_custom(),
          })}
    </p>
  </fieldset>

  {#if failed}<p class="editor-error" role="alert">{m.space_save_failed()}</p>{/if}

  {#if confirming && space}
    <div class="confirm" role="alertdialog" aria-label={m.space_delete_title({ name: space.name })}>
      <p class="confirm-title">{m.space_delete_title({ name: space.name })}</p>
      <p class="confirm-body">{closing(space.tabs)}</p>
      <div class="editor-actions">
        <Button size="compact" variant="secondary" onclick={() => (confirming = false)}
          >{m.space_cancel()}</Button
        >
        <Button size="compact" variant="danger" pending={saving} onclick={remove}
          >{m.space_delete_confirm()}</Button
        >
      </div>
    </div>
  {:else}
    <div class="editor-actions">
      {#if space && spaces.length > 1}<Button
          size="compact"
          variant="ghost"
          class="editor-delete"
          onclick={() => (confirming = true)}>{m.space_delete()}</Button
        >{/if}
      <Button size="compact" variant="secondary" onclick={() => onclose(false)}
        >{m.space_cancel()}</Button
      >
      <Button size="compact" variant="primary" type="submit" pending={saving}
        >{space ? m.space_save() : m.space_create()}</Button
      >
    </div>
  {/if}
</form>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }

  .editor-head {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .editor-back {
    display: grid;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--color-muted);
    place-items: center;
  }

  .editor-back:hover,
  .editor-back:focus-visible {
    outline: none;
    background: var(--row-active);
    color: var(--color-text);
  }

  .editor-title {
    color: var(--color-text);
    font-size: var(--text-body);
    font-weight: 600;
  }

  .editor-name {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px 6px 6px;
    border-radius: var(--radius-row);
    background: var(--row-hover);
    box-shadow: var(--row-rim);
  }

  .editor-name:focus-within {
    background: var(--row-active);
  }

  .editor-name input,
  .emoji-field {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-body);
    font-weight: 500;
    outline: none;
  }

  .editor-group {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    padding: 0 0 7px;
    color: var(--color-faint);
    font-size: var(--text-label);
    font-weight: 500;
  }

  .symbols {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(28px, 1fr));
    gap: 2px;
  }

  .symbol {
    display: grid;
    aspect-ratio: 1;
    padding: 0;
    border: 0;
    border-radius: var(--radius-inset);
    background: transparent;
    color: var(--color-muted);
    place-items: center;
    transition:
      background-color var(--motion-instant) var(--ease-out),
      color var(--motion-instant) var(--ease-out);
  }

  .symbol:hover,
  .symbol:focus-visible {
    outline: none;
    background: var(--row-hover);
    color: var(--color-text);
  }

  .symbol[aria-checked="true"] {
    background: var(--row-active);
    box-shadow: var(--row-rim);
    color: var(--color-text);
  }

  .emoji-field {
    padding: 6px 10px;
    border-radius: var(--radius-control-compact);
    background: var(--row-hover);
    font-weight: 450;
  }

  .emoji-field[data-on] {
    box-shadow: var(--row-rim);
  }

  .colors {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .color {
    --dot-top: var(--avatar-0-top);
    --dot-bottom: var(--avatar-0-bottom);

    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: linear-gradient(180deg, var(--dot-top), var(--dot-bottom));
    box-shadow: inset 0 0 0 0.5px var(--avatar-rim);
    transition: scale var(--motion-fast) var(--ease-spring);
  }

  .color:hover {
    scale: 1.12;
  }

  .color:focus-visible,
  .color[aria-checked="true"] {
    outline: 2px solid var(--color-text);
    outline-offset: 2px;
  }

  .color[data-color="1"] {
    --dot-top: var(--avatar-1-top);
    --dot-bottom: var(--avatar-1-bottom);
  }

  .color[data-color="2"] {
    --dot-top: var(--avatar-2-top);
    --dot-bottom: var(--avatar-2-bottom);
  }

  .color[data-color="3"] {
    --dot-top: var(--avatar-3-top);
    --dot-bottom: var(--avatar-3-bottom);
  }

  .color[data-color="4"] {
    --dot-top: var(--avatar-4-top);
    --dot-bottom: var(--avatar-4-bottom);
  }

  .color[data-color="5"] {
    --dot-top: var(--avatar-5-top);
    --dot-bottom: var(--avatar-5-bottom);
  }

  .color[data-color="6"] {
    --dot-top: var(--avatar-6-top);
    --dot-bottom: var(--avatar-6-bottom);
  }

  .color[data-color="7"] {
    --dot-top: var(--avatar-7-top);
    --dot-bottom: var(--avatar-7-bottom);
  }

  .color[data-color="8"] {
    --dot-top: var(--avatar-8-top);
    --dot-bottom: var(--avatar-8-bottom);
  }

  .color[data-color="9"] {
    --dot-top: var(--avatar-9-top);
    --dot-bottom: var(--avatar-9-bottom);
  }

  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(26px, 1fr));
    gap: 6px;
  }

  .theme {
    position: relative;
    display: grid;
    aspect-ratio: 1;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    place-items: center;
  }

  /* The theme's ground with its accent rising in the corner: what the window
     will be, at the size of a fingertip. */
  .theme-swatch {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    border-radius: 50%;
    background: var(--chip-bg);
    box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--chip-fg) 18%, transparent);
  }

  .theme-swatch i {
    position: absolute;
    width: 55%;
    height: 55%;
    border-radius: 50%;
    background: var(--chip-accent);
    inset-block-end: -12%;
    inset-inline-end: -12%;
  }

  .theme:hover .theme-swatch {
    scale: 1.08;
  }

  .theme:focus-visible,
  .theme[aria-checked="true"] {
    outline: 2px solid var(--color-text);
    outline-offset: 2px;
  }

  .theme-tick {
    position: absolute;
    display: grid;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--chip-fg);
    color: var(--chip-bg);
    place-items: center;
  }

  .theme-note {
    color: var(--color-faint);
    font-size: var(--text-label);
    line-height: 16px;
  }

  .editor-error {
    color: var(--color-danger);
    font-size: var(--text-label);
  }

  .editor-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
  }

  .editor-actions :global(.editor-delete) {
    margin-inline-end: auto;
    color: var(--color-danger);
  }

  .confirm {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border-radius: var(--radius-row);
    background: color-mix(in oklab, var(--color-danger) 10%, transparent);
  }

  .confirm-title {
    color: var(--color-text);
    font-size: var(--text-body);
    font-weight: 600;
  }

  .confirm-body {
    margin-block-end: 4px;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 16px;
  }

  @media (prefers-reduced-motion: reduce) {
    .color,
    .theme-swatch {
      transition: none;
    }
  }
</style>
