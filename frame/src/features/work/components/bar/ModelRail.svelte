<script module lang="ts">
  import type { WorkModelProvider } from "$shared/ipc/bindings";

  /** What the person can do with a provider now: choose its models, or the one step that opens them. */
  export type Standing = "ready" | "key" | "refused" | "sign_in" | "server";
  export type Seat = { provider: WorkModelProvider; standing: Standing };
  /** What the rail shows: one provider's models, or the ones the person starred from all of them. */
  export type View = "favourites" | WorkModelProvider;
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import { providerName } from "$domain/ai";
  import Icon from "$shared/ui/Icon";
  import ProviderMark from "$shared/ui/ProviderMark";
  import ZephiumTile from "$shared/ui/ZephiumTile";
  import * as m from "$shared/i18n/messages";
  import { StarIcon } from "../../lib/icons";

  let {
    seats,
    starred = false,
    current,
    hits = null,
    panel,
    onpick,
  }: {
    seats: readonly Seat[];
    /** The person has starred a model: the favourites seat leads the rail. */
    starred?: boolean;
    current: View | null;
    /** The providers a search matched; null while nothing is searched. */
    hits?: ReadonlySet<WorkModelProvider> | null;
    /** The id of what the rail's selection shows. */
    panel: string;
    onpick: (view: View) => void;
  } = $props();

  const uid = $props.id();

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

  type Stop = { view: View; seat: Seat | null };
  const stops = $derived<Stop[]>([
    ...(starred ? [{ view: "favourites" as const, seat: null }] : []),
    ...seats.map((seat) => ({ view: seat.provider, seat })),
  ]);
  const at = $derived(
    Math.max(
      0,
      stops.findIndex((stop) => stop.view === current),
    ),
  );
  /** The seats that lead (favourites, Zephium's own) stand apart from the providers by a hairline. */
  const lead = $derived((starred ? 1 : 0) + (seats[0]?.provider === "cloud" ? 1 : 0));

  let list = $state<HTMLElement>();
  let plate = $state<{ y: number; h: number; glide: boolean } | null>(null);
  $effect(() => {
    void stops;
    const host = list;
    const seat = host?.querySelectorAll<HTMLElement>('[role="tab"]')[at];
    if (!host || !seat) return;
    const before = untrack(() => plate);
    plate = { y: seat.offsetTop, h: seat.offsetHeight, glide: !!before };
  });

  function key(event: KeyboardEvent) {
    const steps: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
    let to: number;
    if (event.key === "Home") to = 0;
    else if (event.key === "End") to = stops.length - 1;
    else if (event.key in steps)
      to = Math.max(0, Math.min(stops.length - 1, at + steps[event.key]!));
    else return;
    event.preventDefault();
    const next = stops[to];
    if (!next || to === at) return;
    onpick(next.view);
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
  data-search={hits ? "" : undefined}
>
  {#if plate}<span
      class="plate"
      data-glide={plate.glide ? "" : undefined}
      aria-hidden="true"
      style:transform="translateY({plate.y}px)"
      style:block-size="{plate.h}px"
    ></span>{/if}
  {#each stops as stop, index (stop.view)}
    {#if lead > 0 && index === lead}<span class="rule" role="presentation"></span>{/if}
    {@const seat = stop.seat}
    <button
      type="button"
      role="tab"
      class="seat"
      id="{uid}-{stop.view}"
      title={seat ? caption(seat) : m.model_favourites()}
      aria-label={seat ? caption(seat) : m.model_favourites()}
      aria-selected={stop.view === current}
      aria-controls={panel}
      tabindex={stop.view === current ? 0 : -1}
      data-provider={stop.view}
      data-standing={seat?.standing ?? "ready"}
      data-miss={hits && (!seat || !hits.has(seat.provider)) ? "" : undefined}
      style:--ink={seat ? INKS[seat.provider] : undefined}
      onpointerdown={(event) => event.preventDefault()}
      onclick={() => onpick(stop.view)}
    >
      <span class="mark" aria-hidden="true"
        >{#if !seat}<span class="star"><Icon icon={StarIcon} size={16} strokeWidth={1.8} /></span
          >{:else if seat.provider === "cloud"}<ZephiumTile size={24} />{:else}<ProviderMark
            provider={seat.provider}
            size={18}
          />{/if}</span
      >
      {#if seat?.standing === "refused"}<span class="badge" aria-hidden="true"></span>{/if}
    </button>
  {/each}
</div>

<style>
  /* A thin pill of round seats. It is not lit: the light belongs to the card and the chip. */
  .rail {
    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    box-sizing: border-box;
    inline-size: 42px;
    margin: 8px 0 8px 8px;
    padding: 5px;
    align-self: stretch;
    contain: size;
    overflow-y: auto;
    overscroll-behavior: contain;
    border-radius: var(--radius-capsule);
    background: var(--color-fill);
    outline: none;
    scrollbar-width: none;
  }

  .plate {
    position: absolute;
    inset-block-start: 0;
    inset-inline: 5px;
    border-radius: 50%;
    background: var(--row-active);
    pointer-events: none;
  }

  .plate[data-glide] {
    transition:
      transform var(--motion-page) var(--ease-spring),
      block-size var(--motion-base) var(--ease-out);
  }

  .rail[data-search] .plate {
    opacity: 0.4;
  }

  .seat {
    position: relative;
    display: grid;
    flex: none;
    inline-size: 32px;
    block-size: 32px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--ink, var(--color-text));
    cursor: default;
    outline: none;
    place-items: center;
    transition:
      opacity var(--motion-fast) var(--ease-out),
      background-color var(--motion-fast) var(--ease-out),
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

  .star {
    display: grid;
    color: var(--color-label-secondary);
  }

  .star :global(path) {
    fill: currentcolor;
  }

  /* A step still to take is a softer mark, in its own colour; only a refused key is an alert. */
  .seat:not([data-standing="ready"]) .mark {
    opacity: 0.55;
    transition: opacity var(--motion-fast) var(--ease-out);
  }

  .seat:not([data-standing="ready"]):is(:hover, [aria-selected="true"]) .mark {
    opacity: 1;
  }

  .badge {
    position: absolute;
    inset-block-end: 5px;
    inset-inline-end: 5px;
    inline-size: 6px;
    block-size: 6px;
    border-radius: 50%;
    background: var(--color-danger);
  }

  .rule {
    flex: none;
    inline-size: 16px;
    block-size: 1px;
    margin-block: 3px;
    background: var(--color-border-strong, var(--color-border));
    opacity: 0.7;
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
