<script lang="ts">
  import Button from "$shared/ui/Button";
  import HostGlyph from "../cards/HostGlyph.svelte";
  import { cardCountdown, needTitle, type HumanPage } from "../../lib/work-human";
  import * as m from "$shared/i18n/messages";

  /**
   * A page the run waits on, over its frame: what the person is asked to do
   * and where, how long the run still waits, and the one action that meets
   * it. Calm by design: the frame stays visible, only the button is lit.
   */
  let {
    page,
    host,
    url,
    onopen,
    onwait,
  }: {
    page: HumanPage;
    host: string;
    url: string;
    onopen?: () => void;
    onwait?: () => void;
  } = $props();
  const title = $derived(needTitle(page.reason, host));
  const left = $derived(cardCountdown(page.remaining));
  /** The last stretch says so in the warning tone, never in red: nothing failed yet. */
  const late = $derived(page.remaining < 20_000);
</script>

<div class="needs nodrag nopan" role="group" aria-label={title}>
  <p class="what">
    <span class="mark"><HostGlyph {host} {url} size={16} initial={false} /></span>
    <span class="title">{title}</span>
  </p>
  <div class="row">
    {#if left}<span class="left" class:late role="timer" aria-live="off">{left}</span>{/if}
    <span class="actions">
      {#if page.canExtend && onwait}<Button
          size="compact"
          shape="capsule"
          variant="ghost"
          title={m.work_human_extend_hint()}
          onclick={(event: MouseEvent) => {
            event.stopPropagation();
            onwait?.();
          }}>{m.work_human_extend()}</Button
        >{/if}
      {#if onopen}<Button
          size="compact"
          shape="capsule"
          variant="primary"
          onclick={(event: MouseEvent) => {
            event.stopPropagation();
            onopen?.();
          }}>{m.work_human_open()}</Button
        >{/if}
    </span>
  </div>
</div>

<style>
  .needs {
    display: flex;
    flex-direction: column;
    gap: 8px;
    box-sizing: border-box;
    padding: 10px 10px 10px 12px;
    border-radius: var(--radius-row);
    background: var(--color-float);
    box-shadow: var(--shadow-menu);
    color: var(--color-text);
    cursor: default;
    animation: needs-arrive var(--motion-base) var(--ease-emphasized) both;
  }

  .what {
    display: flex;
    align-items: center;
    gap: 8px;
    min-inline-size: 0;
    margin: 0;
  }

  .mark {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 16px;
    block-size: 16px;
  }

  .title {
    overflow: hidden;
    font-size: var(--text-body);
    font-weight: 600;
    line-height: 18px;
    letter-spacing: -0.005em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-block-size: var(--control-compact);
  }

  .left {
    padding-inline-start: 24px;
    color: var(--color-muted);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
    line-height: 16px;
    transition: color var(--motion-base) var(--ease-out);
  }

  .left.late {
    color: var(--color-warning);
  }

  .actions {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 4px;
    margin-inline-start: auto;
  }

  @keyframes needs-arrive {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .needs {
      animation: none;
    }
  }
</style>
