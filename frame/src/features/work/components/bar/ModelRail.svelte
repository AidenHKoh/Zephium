<script module lang="ts">
  import type { WorkModelProvider } from "$shared/ipc/bindings";

  /** What the person can do with a provider now: choose its models, or the one step that opens them. */
  export type Standing = "ready" | "key" | "refused" | "sign_in" | "server";
  export type Seat = { provider: WorkModelProvider; standing: Standing };
</script>

<script lang="ts">
  import { providerName } from "$domain/ai";
  import ProviderMark from "$shared/ui/ProviderMark";
  import ZephiumTile from "$shared/ui/ZephiumTile";
  import * as m from "$shared/i18n/messages";

  let {
    seats,
    current,
    hits = null,
    panel,
    onpick,
  }: {
    seats: readonly Seat[];
    current: WorkModelProvider | null;
    /** The providers a search matched; null while nothing is searched. */
    hits?: ReadonlySet<WorkModelProvider> | null;
    /** The id of what the rail's selection shows. */
    panel: string;
    onpick: (provider: WorkModelProvider) => void;
  } = $props();

  const uid = $props.id();
  /** The hairline, and its margins, between what is ready and what is not. */
  const RULE = 9;

  /** Brand colour for the marks that are a single ink; the rest carry their own. */
  const INKS: Partial<Record<WorkModelProvider, string>> = {
    open_router: "#6467f2",
    compatible: "var(--color-label-secondary)",
  };

  const names = (provider: WorkModelProvider) =>
    provider === "cloud"
      ? m.ai_cloud_title()
      : provider === "compatible"
        ? m.ai_custom_name()
        : providerName(provider);

  function caption(seat: Seat) {
    const name = names(seat.provider);
    switch (seat.standing) {
      case "ready":
        return name;
      case "refused":
        return m.model_rail_refused({ provider: name });
      case "sign_in":
        return m.model_rail_sign_in({ provider: name });
      case "server":
        return m.model_rail_server({ provider: name });
      default:
        return m.model_rail_key({ provider: name });
    }
  }

  const at = $derived(
    Math.max(
      0,
      seats.findIndex((seat) => seat.provider === current),
    ),
  );
  const split = $derived(seats.findIndex((seat) => seat.standing !== "ready"));
  const gap = $derived(split > 0 && at >= split ? RULE : 0);

  let list = $state<HTMLElement>();

  function key(event: KeyboardEvent) {
    const steps: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
    let to: number;
    if (event.key === "Home") to = 0;
    else if (event.key === "End") to = seats.length - 1;
    else if (event.key in steps)
      to = Math.max(0, Math.min(seats.length - 1, at + steps[event.key]!));
    else return;
    event.preventDefault();
    const next = seats[to];
    if (!next || to === at) return;
    onpick(next.provider);
    list?.querySelectorAll<HTMLElement>('[role="tab"]')[to]?.focus();
  }
</script>

<div
  class="rail"
  bind:this={list}
  role="tablist"
  aria-orientation="vertical"
  aria-label={m.model_providers()}
  tabindex="-1"
  onkeydown={key}
  style:--at={at}
  style:--gap="{gap}px"
  data-search={hits ? "" : undefined}
>
  <span class="plate" aria-hidden="true"></span>
  {#each seats as seat, index (seat.provider)}
    {#if index === split && split > 0}<span class="rule" role="presentation"></span>{/if}
    <button
      type="button"
      role="tab"
      class="seat"
      id="{uid}-{seat.provider}"
      title={caption(seat)}
      aria-label={caption(seat)}
      aria-selected={seat.provider === current}
      aria-controls={panel}
      tabindex={seat.provider === current ? 0 : -1}
      data-provider={seat.provider}
      data-standing={seat.standing}
      data-miss={hits && !hits.has(seat.provider) ? "" : undefined}
      style:--ink={INKS[seat.provider]}
      onpointerdown={(event) => event.preventDefault()}
      onclick={() => onpick(seat.provider)}
    >
      <span class="mark" aria-hidden="true"
        >{#if seat.provider === "cloud"}<ZephiumTile size={21} />{:else}<ProviderMark
            provider={seat.provider}
            size={20}
          />{/if}</span
      >
      {#if seat.standing !== "ready"}<span class="badge" aria-hidden="true"></span>{/if}
    </button>
  {/each}
</div>

<style>
  .rail {
    --pitch: 38px;

    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    gap: 2px;
    box-sizing: border-box;
    inline-size: 48px;
    margin: 6px 0 6px 6px;
    padding: 4px;
    overflow-y: auto;
    overscroll-behavior: contain;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    background: var(--color-fill);
    outline: none;
    scrollbar-width: none;
  }

  .plate {
    position: absolute;
    inset-block-start: 4px;
    inset-inline: 4px;
    block-size: 36px;
    border-radius: var(--radius-row);
    background: var(--row-active);
    box-shadow: var(--shine-top);
    pointer-events: none;
    transform: translateY(calc(var(--at) * var(--pitch) + var(--gap)));
    transition:
      transform var(--motion-page) var(--ease-spring),
      opacity var(--motion-base) var(--ease-out);
  }

  .rail[data-search] .plate {
    opacity: 0.4;
  }

  .seat {
    position: relative;
    display: grid;
    flex: none;
    inline-size: 40px;
    block-size: 36px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--ink, var(--color-text));
    cursor: default;
    outline: none;
    place-items: center;
    transition:
      opacity var(--motion-fast) var(--ease-out),
      scale var(--motion-base) var(--ease-spring);
  }

  .seat:active {
    scale: 0.92;
  }

  .seat:focus-visible {
    box-shadow: inset 0 0 0 1.5px var(--color-ring);
  }

  .seat:hover:not([aria-selected="true"]) {
    background: color-mix(in oklab, var(--row-active) 55%, transparent);
  }

  .seat[data-miss] {
    opacity: 0.3;
  }

  .mark {
    display: grid;
    place-items: center;
  }

  /* A step still to take, quietly: the mark keeps its colour and a hollow dot says it is not connected. */
  .badge {
    position: absolute;
    inset-block-end: 5px;
    inset-inline-end: 4px;
    inline-size: 6px;
    block-size: 6px;
    box-sizing: border-box;
    border: 1.2px solid var(--color-faint);
    border-radius: 50%;
    background: var(--color-float);
  }

  .seat[data-standing="refused"] .badge {
    border-color: var(--color-danger);
    background: var(--color-danger);
  }

  .rule {
    flex: none;
    block-size: 1px;
    margin: 4px 8px;
    background: var(--color-border);
  }

  @media (prefers-reduced-motion: reduce) {
    .plate,
    .seat {
      transition: none;
    }
  }

  @media (forced-colors: active) {
    .plate {
      outline: 1px solid Highlight;
    }
  }
</style>
