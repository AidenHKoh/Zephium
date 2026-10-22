<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import { Delete02Icon, PaintBoardIcon } from "@hugeicons/core-free-icons";
  import IconButton from "$shared/ui/IconButton";
  import { commands, type PageStyleStateV1 } from "$shared/ipc/bindings";
  import "./assist.css";

  let {
    site,
    summary,
    state,
    enabled = true,
    ondecide,
    ontoggle,
    onremove,
  }: {
    site: string;
    summary: string;
    state: PageStyleStateV1;
    /** Whether the applied style is on now; the core's word, not a guess. */
    enabled?: boolean;
    ondecide: (approve: boolean) => void;
    ontoggle: (enabled: boolean) => void;
    onremove: () => void;
  } = $props();
</script>

{#if state === "proposed"}
  <section class="assist-card assist-rise" aria-label={m.assist_style_title()}>
    <div class="head">
      <span class="plate" aria-hidden="true"><Icon icon={PaintBoardIcon} size={15} /></span>
      <div class="words">
        <h4>{m.assist_style_title()}</h4>
        <p>{m.assist_style_scope({ site })}</p>
      </div>
    </div>
    <p class="change">{summary}</p>
    <div class="assist-actions">
      <Button variant="primary" size="compact" onclick={() => ondecide(true)}
        >{m.assist_style_apply()}</Button
      >
      <Button size="compact" onclick={() => ondecide(false)}>{m.assist_style_discard()}</Button>
    </div>
  </section>
{:else if state === "applied"}
  <div class="badge assist-rise" data-on={enabled}>
    <span class="dot" aria-hidden="true"></span>
    <span class="label">{enabled ? m.assist_style_on({ site }) : m.assist_style_off({ site })}</span
    >
    <Button variant="ghost" size="compact" onclick={() => ontoggle(!enabled)}
      >{enabled ? m.assist_style_turn_off() : m.assist_style_turn_on()}</Button
    >
    <IconButton
      icon={Delete02Icon}
      size={13}
      buttonSize={24}
      label={m.assist_style_remove()}
      onclick={onremove}
    />
  </div>
{:else if state === "full"}
  <div class="full assist-rise">
    <p>{m.assist_style_full()}</p>
    <Button
      variant="ghost"
      size="compact"
      onclick={() => void commands.runCommand("settings.sites").catch(() => {})}
      >{m.assist_style_full_open()}</Button
    >
  </div>
{:else}
  <p class="gone assist-rise">
    {state === "rejected" ? m.assist_style_failed() : m.assist_style_discarded()}
  </p>
{/if}

<style>
  .head {
    display: flex;
    gap: 10px;
  }

  .plate {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 28px;
    block-size: 28px;
    border-radius: var(--radius-inset);
    background: var(--color-field);
    color: var(--color-muted);
  }

  .words {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-inline-size: 0;
  }

  .assist-card .change {
    padding: 8px 10px;
    border-radius: var(--radius-inset);
    background: var(--color-field);
    color: var(--color-label-secondary);
    font-size: 12.5px;
    line-height: 1.45;
  }

  .badge {
    display: flex;
    align-items: center;
    gap: 8px;
    box-sizing: border-box;
    min-block-size: 34px;
    padding: 4px 4px 4px 12px;
    border-radius: var(--radius-row);
    background: var(--color-fill);
    box-shadow: inset 0 0 0 1px var(--color-border);
    color: var(--color-muted);
    font-size: 12.5px;
  }

  .dot {
    flex: none;
    inline-size: 6px;
    block-size: 6px;
    border-radius: var(--radius-capsule);
    background: var(--color-faint);
  }

  .badge[data-on="true"] .dot {
    background: var(--color-success);
  }

  .label {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .full {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }

  .full :global(.ui-button) {
    margin-inline-start: -10px;
  }

  .full p {
    margin: 0;
    color: var(--color-muted);
    font-size: 12.5px;
    line-height: 1.45;
    text-wrap: pretty;
  }

  .gone {
    margin: 0;
    color: var(--color-faint);
    font-size: 12.5px;
  }
</style>
