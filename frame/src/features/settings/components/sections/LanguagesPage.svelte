<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { preferences } from "$domain/preferences";
  import { interfaceLanguages, systemLocale } from "$shared/lib/locale.svelte";
  import { getLocale } from "$shared/i18n/runtime";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import Icon from "$shared/ui/Icon";
  import { Tick02Icon } from "@hugeicons/core-free-icons";

  /** A language in its own words, and in the words the interface shows. */
  function names(code: string, shown: string) {
    const name = (inLocale: string) => {
      try {
        return new Intl.DisplayNames([inLocale], { type: "language" }).of(code) ?? code;
      } catch {
        return code;
      }
    };
    const own = name(code);
    return { own: own.charAt(0).toLocaleUpperCase(code) + own.slice(1), shown: name(shown) };
  }

  let shown = $derived(getLocale() as string);
  let chosen = $derived(preferences.value("ui.language") || "system");
  let options = $derived([
    { value: "system", title: m.language_system(), detail: names(systemLocale(), shown).shown },
    ...interfaceLanguages()
      .map((code) => {
        const { own, shown: translated } = names(code, shown);
        return { value: code, title: own, detail: translated === own ? "" : translated };
      })
      .sort((a, b) => a.title.localeCompare(b.title)),
  ]);

  let list = $state<HTMLElement>();
  function choose(value: string) {
    if (value !== chosen) void preferences.set("ui.language", value);
  }
  function onkeydown(event: KeyboardEvent) {
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
    if (!step) return;
    event.preventDefault();
    const index = options.findIndex((option) => option.value === chosen);
    const next = options[(index + step + options.length) % options.length]!;
    choose(next.value);
    list?.querySelector<HTMLElement>(`[data-value="${next.value}"]`)?.focus();
  }
</script>

<SettingsGroup title={m.language_interface_title()} description={m.language_websites_note()}>
  <div
    class="languages"
    role="radiogroup"
    aria-label={m.language_interface_title()}
    data-setting="languages.interface"
    bind:this={list}
  >
    {#each options as option (option.value)}
      {@const selected = option.value === chosen}
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        tabindex={selected ? 0 : -1}
        data-value={option.value}
        disabled={preferences.saving()}
        onclick={() => choose(option.value)}
        {onkeydown}
        ><span class="copy"
          ><span class="title" lang={option.value === "system" ? undefined : option.value}
            >{option.title}</span
          >{#if option.detail}<span class="detail">{option.detail}</span>{/if}</span
        >{#if selected}<Icon icon={Tick02Icon} size={16} />{/if}</button
      >
    {/each}
  </div>
</SettingsGroup>

<style>
  .languages {
    display: flex;
    flex-direction: column;
    padding: 6px;
  }

  button {
    display: flex;
    align-items: center;
    gap: 12px;
    min-block-size: 44px;
    padding-inline: 12px;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--color-text);
    text-align: start;
    cursor: default;
    transition: background-color var(--motion-fast) var(--ease-out);
  }

  button:hover {
    background: var(--row-hover);
  }

  button[aria-checked="true"] {
    background: var(--row-active);
  }

  button:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: -2px;
  }

  .copy {
    display: flex;
    flex: 1;
    align-items: baseline;
    gap: 8px;
    min-inline-size: 0;
  }

  .title {
    font-size: 13px;
    font-weight: 500;
    letter-spacing: -0.006em;
  }

  .detail {
    overflow: hidden;
    color: var(--color-muted);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  button[aria-checked="true"] :global(svg) {
    color: var(--color-text);
  }
</style>
