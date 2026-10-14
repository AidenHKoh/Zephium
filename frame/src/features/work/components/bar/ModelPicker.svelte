<script lang="ts">
  import { Popover } from "bits-ui";
  import { onMount, tick, untrack } from "svelte";
  import { accountSession } from "$domain/account";
  import type { WorkModelEntry, WorkModelProvider, WorkModelReasoning } from "$shared/ipc/bindings";
  import { commands } from "$shared/ipc/bindings";
  import {
    ModelsSession,
    contextSize,
    effortLevels,
    effortName,
    entryOf,
    keyState,
    locked,
    pickerGroups,
    providerName,
    searchEntries,
    shortName,
    usable,
  } from "$domain/ai";
  import Icon from "$shared/ui/Icon";
  import ProviderMark from "$shared/ui/ProviderMark";
  import { menuMaterialFor } from "$shared/ui/Menu/material";
  import { Key01Icon, Search01Icon, Tick02Icon } from "../../lib/icons";
  import * as m from "$shared/i18n/messages";
  import { EFFORTS, nearestEffort, stepEffort } from "./ModelEffort.svelte";
  import ModelHero, { type HeroSubject, type Offer } from "./ModelHero.svelte";
  import ModelTier, { tiers, type Tier } from "./ModelTier.svelte";
  import "$shared/ui/Menu/popover.css";

  let {
    profile,
    onsettings = () => void commands.runCommand("settings.ai"),
  }: {
    profile: string;
    /** Opens Settings → AI, where keys are added. */
    onsettings?: () => void;
  } = $props();

  const uid = $props.id();

  let session = $state.raw<ModelsSession | null>(null);
  $effect(() => {
    const owner = new ModelsSession(profile);
    session = owner;
    void owner.start();
    return () => owner.dispose();
  });

  onMount(() => {
    void accountSession.retain();
    return () => accountSession.release();
  });
  const account = $derived(accountSession.account());
  /** Hosted models this server offers to someone not yet signed in. */
  const hostedOffer = $derived(
    !!account?.available && !account.signed_in && account.hosted_models === true,
  );

  let open = $state(false);
  let trigger = $state<HTMLElement | null>(null);
  let material = $state<"opaque" | undefined>();
  let list = $state<HTMLElement>();
  let field = $state<HTMLInputElement>();

  const models = $derived(session?.models ?? null);
  const current = $derived(entryOf(models, models?.effective.lead));
  const chosen = $derived(models?.effective.lead ?? null);
  const groups = $derived(pickerGroups(models, "lead", session?.endpoint ?? []));
  const allKeyed = $derived(groups.needsKey.length === 0);
  const tierOf = $derived(tiers(groups.ready));

  type Item =
    | { id: string; kind: "model"; entry: WorkModelEntry }
    | { id: string; kind: "offer"; provider: WorkModelProvider; offer: Offer };
  type Section = { key: string; label: string; provider: WorkModelProvider | null; items: Item[] };

  const offers = $derived<Item[]>([
    ...(hostedOffer
      ? [
          {
            id: "offer:cloud",
            kind: "offer" as const,
            provider: "cloud" as const,
            offer: "sign_in" as const,
          },
        ]
      : []),
    ...groups.needsKey.map((provider) => ({
      id: `offer:${provider}`,
      kind: "offer" as const,
      provider,
      offer: keyState(models, provider) === "invalid" ? ("refused" as const) : ("key" as const),
    })),
  ]);

  let query = $state("");
  const searching = $derived(query.trim().length > 0);
  const sections = $derived.by<Section[]>(() => {
    const matched = searching
      ? new Set(searchEntries(groups.ready, query).map((entry) => entry.id))
      : null;
    const ready = groups.ready
      .map((group) => ({
        key: group.provider,
        label: group.provider === "cloud" ? m.ai_cloud_title() : providerName(group.provider),
        provider: group.provider,
        items: group.entries
          .filter((entry) => !matched || matched.has(entry.id))
          .map((entry) => ({ id: entry.id, kind: "model" as const, entry })),
      }))
      .filter((section) => section.items.length);
    if (searching || !offers.length) return ready;
    return [...ready, { key: "offers", label: m.model_offers(), provider: null, items: offers }];
  });
  const items = $derived(sections.flatMap((section) => section.items));
  const slot = (id: string) => `${uid}-${items.findIndex((item) => item.id === id)}`;

  /** What the hero shows and the plate marks: the pointer's or the arrows' row, else the running model. */
  let active = $state<string | null>(null);
  const home = $derived(items.find((item) => item.id === chosen)?.id ?? items[0]?.id ?? null);
  const focus = $derived(items.find((item) => item.id === active) ?? null);

  /** The effort the person last set while the menu is open, ahead of Rust's answer. */
  let intent = $state<WorkModelReasoning | null>(null);
  const levels = $derived(effortLevels(models, chosen));
  const effort = $derived<WorkModelReasoning | null>(
    levels.length ? (intent ? nearestEffort(levels, intent) : (models?.effort ?? null)) : null,
  );

  const hero = $derived.by<HeroSubject | null>(() => {
    const item = focus ?? items.find((row) => row.id === home) ?? null;
    if (item?.kind === "offer")
      return { kind: "offer", provider: item.provider, offer: item.offer };
    const entry = item?.entry ?? current;
    if (!entry) return null;
    const own = effortLevels(models, entry.id);
    return {
      kind: "model",
      entry,
      tier: tierOf[entry.id] ?? null,
      levels: own,
      effort:
        entry.id === chosen
          ? effort
          : own.length
            ? nearestEffort(own, intent ?? models?.effort ?? "medium")
            : null,
    };
  });

  /**
   * Whether a run could start: the lead needs a model with a working key. Asked
   * again whenever the models change, so a key added in Settings lands here.
   */
  let leadMissing = $state(false);
  /** No chosen model can search and no OpenAI key stands in: runs work without the web. */
  let searchOff = $state(false);
  $effect(() => {
    void models;
    let live = true;
    void Promise.resolve()
      .then(() => commands.workModelsReady(profile))
      .then((readiness) => {
        if (!live) return;
        leadMissing = !readiness.ready && readiness.missing_roles.includes("lead");
        searchOff = readiness.ready && readiness.search === "unavailable";
      })
      .catch(() => {
        if (live) {
          leadMissing = false;
          searchOff = false;
        }
      });
    return () => (live = false);
  });
  /** No model can run yet: the one thing to do is add a key, so the control is that, not a menu. */
  const keyless = $derived(!!models && leadMissing);

  function reset() {
    query = "";
    active = null;
    intent = null;
  }

  async function opened(next: boolean) {
    open = next;
    if (!next) {
      reset();
      return;
    }
    material = menuMaterialFor(trigger);
    // The person's own server says which models it serves; the rest is the curated list.
    if (usable(models, "compatible")) void session?.listEndpoint();
    active = home;
    await tick();
    field?.focus();
  }

  /** The menu stays while the choice lands, so the hero and the trigger change before it closes. */
  async function choose(entry: WorkModelEntry) {
    if (locked(models, entry.id) || session?.busy) return;
    if (entry.id !== chosen && !(await session?.choose("lead", entry.id))) return;
    open = false;
    reset();
  }

  /** Each change goes to Rust in turn; a newer one waits for the last to land. */
  let wanted: WorkModelReasoning | null = null;
  let sending = false;
  async function setEffort(level: WorkModelReasoning) {
    intent = level;
    wanted = level;
    if (sending) return;
    sending = true;
    try {
      while (session && wanted && wanted !== models?.effort) {
        const asked: WorkModelReasoning = wanted;
        if (!(await session.setEffort(asked))) {
          if (session.busy) {
            await new Promise((done) => setTimeout(done, 60));
            continue;
          }
          wanted = null;
          intent = null;
          break;
        }
        if (wanted === asked) wanted = null;
      }
    } finally {
      sending = false;
    }
  }

  function settings() {
    open = false;
    reset();
    onsettings();
  }

  function act(item: Item) {
    if (item.kind === "model") void choose(item.entry);
    else if (item.offer === "sign_in") {
      open = false;
      reset();
      void commands.runCommand("settings.account");
    } else settings();
  }

  function shown(id: string | null) {
    if (!id) return;
    list?.querySelector(`[data-id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function walk(to: number) {
    const next = items[Math.max(0, Math.min(items.length - 1, to))];
    if (!next) return;
    active = next.id;
    shown(next.id);
  }

  function retyped() {
    active = items.find((item) => item.kind === "model")?.id ?? items[0]?.id ?? null;
    void tick().then(() => shown(active));
  }

  function keys(event: KeyboardEvent) {
    if (event.defaultPrevented || event.isComposing) return;
    const target = event.target as HTMLElement;
    const inField = target === field;
    const onButton = target instanceof HTMLButtonElement;
    if (
      !inField &&
      !onButton &&
      event.key.length === 1 &&
      event.key !== " " &&
      !event.metaKey &&
      !event.ctrlKey &&
      !event.altKey
    ) {
      event.preventDefault();
      query += event.key;
      field?.focus();
      retyped();
      return;
    }
    if (onButton && (event.key === "Enter" || event.key === " ")) return;
    if (target.getAttribute("role") === "slider" && event.key === "Enter") return;
    const at = items.findIndex((item) => item.id === active);
    switch (event.key) {
      case "ArrowDown":
      case "ArrowUp":
        event.preventDefault();
        walk(at === -1 ? 0 : at + (event.key === "ArrowDown" ? 1 : -1));
        return;
      case "Home":
      case "End":
        if (query) return;
        event.preventDefault();
        walk(event.key === "Home" ? 0 : items.length - 1);
        return;
      case "ArrowLeft":
      case "ArrowRight": {
        if (query || hero?.kind !== "model" || !hero.effort) return;
        event.preventDefault();
        const rtl = getComputedStyle(target).direction === "rtl";
        const next = stepEffort(
          hero.levels,
          hero.effort,
          (event.key === "ArrowRight") !== rtl ? 1 : -1,
        );
        if (next) void setEffort(next);
        return;
      }
      case "Enter": {
        const item = focus ?? items[0];
        if (!item) return;
        event.preventDefault();
        act(item);
        return;
      }
    }
  }

  function escape(event: KeyboardEvent) {
    if (!query) return;
    event.preventDefault();
    query = "";
    active = home;
    field?.focus();
  }

  function pointed(event: PointerEvent) {
    const id = (event.target as Element).closest<HTMLElement>("[data-id]")?.dataset.id;
    if (id && id !== active) active = id;
  }

  function clicked(event: MouseEvent) {
    const id = (event.target as Element).closest<HTMLElement>("[data-id]")?.dataset.id;
    const item = items.find((row) => row.id === id);
    if (item) act(item);
  }

  /** One plate glides under the row the hero shows, rather than each row lighting on its own. */
  let plate = $state<{ y: number; h: number; glide: boolean } | null>(null);
  $effect(() => {
    void items;
    const id = active;
    const host = list;
    const at = id ? host?.querySelector<HTMLElement>(`[data-id="${CSS.escape(id)}"]`) : null;
    const before = untrack(() => plate);
    plate = at ? { y: at.offsetTop, h: at.offsetHeight, glide: !!before } : null;
  });

  const triggerLabel = $derived(
    current
      ? effort
        ? m.model_trigger_effort({ model: current.display_name, effort: effortName(effort) })
        : m.work_model_trigger({ model: current.display_name })
      : m.work_model_label(),
  );
</script>

{#snippet dots(levels: readonly WorkModelReasoning[], level: WorkModelReasoning)}
  {@const at = levels.indexOf(level)}
  <span class="dots" data-level={level} aria-hidden="true"
    >{#each levels as stop, step (stop)}<span
        class="dot"
        data-on={step <= at ? "" : undefined}
        style:--d="{2.5 + EFFORTS.indexOf(stop) * 0.25}px"
        style:--rise="{EFFORTS.indexOf(stop) * 1.6}px"
      ></span>{/each}</span
  >
{/snippet}

{#snippet tierMark(tier: Tier | undefined)}
  {#if tier}<span class="row-tier"
      ><ModelTier {tier} /><span class="sr"
        >{tier === "light"
          ? m.model_tier_light()
          : tier === "balanced"
            ? m.model_tier_balanced()
            : m.model_tier_flagship()}</span
      ></span
    >{/if}
{/snippet}

{#if keyless}
  <button type="button" class="model-trigger add-key" onclick={settings}>
    <span class="trigger-mark" aria-hidden="true"><Icon icon={Key01Icon} size={14} /></span>
    <span class="trigger-name"
      >{hostedOffer ? m.work_model_start() : m.work_model_add_model_key()}</span
    >
  </button>
{:else}<Popover.Root {open} onOpenChange={(next) => void opened(next)}>
    <Popover.Trigger
      bind:ref={trigger}
      class="model-trigger"
      aria-label={triggerLabel}
      disabled={!models}
    >
      {#if current}
        <span class="trigger-mark" aria-hidden="true"
          ><ProviderMark provider={current.model.provider} size={15} /></span
        >
        <span class="trigger-name">{shortName(current)}</span>
        {#if effort && levels.length > 1}{@render dots(levels, effort)}{/if}
      {:else}
        <span class="trigger-name empty">{m.work_model_none()}</span>
      {/if}
    </Popover.Trigger>
    <Popover.Portal>
      <Popover.Content
        class="ui-menu model-menu"
        data-menu-material={material}
        side="top"
        align="end"
        sideOffset={10}
        collisionPadding={12}
        aria-label={m.work_model_label()}
        onOpenAutoFocus={(event) => event.preventDefault()}
        onEscapeKeydown={escape}
        onkeydown={keys}
      >
        {#if hero}<ModelHero subject={hero} onEffort={(level) => void setEffort(level)} />{/if}
        <div class="find" data-open={searching ? "" : undefined}>
          <label class="find-row">
            <span class="find-icon" aria-hidden="true"
              ><Icon icon={Search01Icon} size={13} strokeWidth={2} /></span
            >
            <input
              bind:this={field}
              bind:value={query}
              type="text"
              role="combobox"
              spellcheck="false"
              autocomplete="off"
              aria-label={m.model_search()}
              aria-expanded="true"
              aria-controls="{uid}-list"
              aria-autocomplete="list"
              aria-activedescendant={active && items.some((item) => item.id === active)
                ? slot(active)
                : undefined}
              oninput={retyped}
            />
          </label>
        </div>
        <div
          class="list"
          id="{uid}-list"
          role="listbox"
          aria-label={searching ? m.model_search() : m.work_model_label()}
          tabindex="-1"
          bind:this={list}
          onpointermove={pointed}
          onpointerleave={() => (active = home)}
          onpointerdown={(event) => event.preventDefault()}
          onclick={clicked}
          onkeydown={keys}
        >
          {#if plate}<span
              class="plate"
              data-glide={plate.glide ? "" : undefined}
              aria-hidden="true"
              style:transform="translateY({plate.y}px)"
              style:block-size="{plate.h}px"
            ></span>{/if}
          {#each sections as section (section.key)}
            <div class="section" role="group" aria-labelledby="{uid}-{section.key}">
              <div class="label" id="{uid}-{section.key}">
                {#if section.provider}<span class="label-mark" aria-hidden="true"
                    ><ProviderMark provider={section.provider} size={12} /></span
                  >{/if}<span>{section.label}</span>
              </div>
              {#each section.items as item (item.id)}
                {#if item.kind === "model"}
                  {@const closed = locked(models, item.entry.id)}
                  {@const small = item.entry.context_window < 500_000}
                  <div
                    class="option"
                    id={slot(item.id)}
                    role="option"
                    data-id={item.id}
                    data-active={item.id === active ? "" : undefined}
                    aria-selected={item.id === chosen}
                    aria-disabled={closed || !!session?.busy || undefined}
                  >
                    <span class="check" aria-hidden="true"
                      >{#if item.id === chosen}<Icon
                          icon={Tick02Icon}
                          size={13}
                          strokeWidth={2.2}
                        />{/if}</span
                    >
                    <span class="option-name">{item.entry.display_name}</span>
                    {#if closed}<span class="aside">{m.work_model_locked()}</span>{:else}
                      {#if small}<span class="aside">{contextSize(item.entry.context_window)}</span
                        >{/if}
                      {@render tierMark(tierOf[item.entry.id])}
                    {/if}
                  </div>
                {:else}
                  <div
                    class="option offer"
                    id={slot(item.id)}
                    role="option"
                    data-id={item.id}
                    data-active={item.id === active ? "" : undefined}
                    data-refused={item.offer === "refused" ? "" : undefined}
                    aria-selected="false"
                  >
                    <span class="check" aria-hidden="true"
                      ><ProviderMark provider={item.provider} size={14} /></span
                    >
                    <span class="option-name"
                      >{item.provider === "cloud"
                        ? m.ai_cloud_title()
                        : providerName(item.provider)}</span
                    >
                    <span class="aside action"
                      >{item.offer === "sign_in"
                        ? m.work_model_sign_in()
                        : item.offer === "refused"
                          ? m.work_model_key_refused()
                          : m.model_add_key()}</span
                    >
                  </div>
                {/if}
              {/each}
            </div>
          {/each}
          {#if searching && !items.length}<p class="empty">
              {m.model_no_match({ query: query.trim() })}
            </p>{/if}
        </div>
        {#if searchOff}<p class="note">{m.work_model_no_search()}</p>{/if}
        {#if session?.fault?.action === "choose:lead"}<p class="note failure" role="alert">
            {m.work_model_failed()}
          </p>{/if}
        {#if session?.fault?.action === "effort"}<p class="note failure" role="alert">
            {m.model_effort_failed()}
          </p>{/if}
        <div class="foot">
          <button type="button" class="link" onclick={settings}
            >{allKeyed ? m.work_model_manage() : m.work_model_add_key()}</button
          >
          {#if !searching}<span class="type" aria-hidden="true">{m.model_type_to_search()}</span
            >{/if}
        </div>
      </Popover.Content>
    </Popover.Portal>
  </Popover.Root>{/if}

<style>
  /* Calm, not an alarm: the model's place, asking for the key it needs. */
  .add-key {
    color: var(--color-label-secondary);
  }

  :global(.model-trigger) {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-inline-size: 220px;
    block-size: 28px;
    margin-block: 3px;
    padding-inline: 8px 9px;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--color-muted);
    font: inherit;
    font-size: var(--text-label);
    cursor: default;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out);
  }

  :global(.model-trigger:hover:not(:disabled)),
  :global(.model-trigger[data-state="open"]) {
    background: var(--color-fill-hover);
    color: var(--color-text);
  }

  :global(.model-trigger:focus-visible) {
    outline: 2px solid var(--color-ring);
    outline-offset: 1px;
  }

  :global(.model-trigger:disabled) {
    opacity: 0;
  }

  .trigger-mark {
    display: grid;
    flex: none;
    place-items: center;
  }

  .trigger-name {
    overflow: hidden;
    font-weight: 500;
    letter-spacing: -0.003em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .trigger-name.empty {
    font-weight: 450;
  }

  /* The hero's dots, small: one per level the model takes, as large as its thought. */
  .dots {
    --alive-a: color-mix(in oklab, var(--color-soft-sky) 66%, var(--color-text));
    --alive-b: color-mix(in oklab, var(--color-soft-lilac) 66%, var(--color-text));

    display: inline-flex;
    flex: none;
    align-items: flex-end;
    gap: 1.5px;
    block-size: 10px;
    margin-inline-start: 1px;
  }

  /* Each dot sits a step above the last, so the row reads as rising rather than as an ellipsis. */
  .dot {
    inline-size: var(--d);
    block-size: var(--d);
    margin-block-end: var(--rise);
    border-radius: 50%;
    background: currentcolor;
    opacity: 0.22;
    transition: opacity var(--motion-base) var(--ease-out);
  }

  .dot[data-on] {
    opacity: 0.95;
  }

  .dots[data-level="max"] .dot[data-on] {
    background: linear-gradient(160deg, var(--alive-a), var(--alive-b));
    opacity: 1;
  }

  :global(.ui-menu.model-menu) {
    display: flex;
    flex-direction: column;
    inline-size: 336px;
    max-block-size: min(540px, var(--bits-floating-available-height, 540px));
    padding: 0;
    transform-origin: var(--bits-floating-transform-origin, bottom right);
    animation:
      ui-menu-fade var(--motion-fast) var(--ease-out),
      model-menu-in var(--motion-page) var(--ease-spring);
  }

  :global(.ui-menu.model-menu[data-state="closed"]) {
    animation: ui-menu-out var(--motion-instant) var(--ease-exit) forwards;
  }

  .find {
    display: grid;
    flex: none;
    grid-template-rows: 0fr;
    margin-inline: 6px;
    opacity: 0;
    transition:
      grid-template-rows var(--motion-base) var(--ease-out),
      opacity var(--motion-fast) var(--ease-out);
  }

  .find[data-open] {
    grid-template-rows: 1fr;
    opacity: 1;
  }

  .find-row {
    display: flex;
    align-items: center;
    gap: 7px;
    min-block-size: 0;
    overflow: hidden;
    padding-inline: 10px;
    color: var(--color-faint);
  }

  .find[data-open] .find-row {
    block-size: 30px;
    margin-block-start: 6px;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    background: var(--color-field);
  }

  .find-icon {
    display: grid;
    flex: none;
    place-items: center;
  }

  .find input {
    flex: 1;
    min-inline-size: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-body);
    caret-color: var(--color-text);
    outline: none;
  }

  .list {
    position: relative;
    flex: 1;
    min-block-size: 0;
    padding: 4px 6px 12px;
    overflow-y: auto;
    overscroll-behavior: contain;
    outline: none;
    scrollbar-width: thin;
    mask-image: linear-gradient(to bottom, black calc(100% - 14px), transparent);
  }

  .plate {
    position: absolute;
    inset-block-start: 0;
    inset-inline: 6px;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    background: var(--row-active);
    pointer-events: none;
  }

  .plate[data-glide] {
    transition:
      transform var(--motion-page) var(--ease-spring),
      block-size var(--motion-base) var(--ease-out);
  }

  .section + .section {
    margin-block-start: 6px;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 4px;
    color: var(--color-faint);
    font-size: var(--text-caption);
    font-weight: 550;
    line-height: 14px;
    letter-spacing: 0.01em;
  }

  .label-mark {
    display: grid;
    inline-size: 14px;
    place-items: center;
    opacity: 0.9;
  }

  .option {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    min-block-size: 32px;
    padding-inline: 10px 12px;
    cursor: default;
  }

  .option[aria-disabled="true"] {
    opacity: 0.45;
  }

  .check {
    display: grid;
    flex: none;
    inline-size: 14px;
    place-items: center;
    color: var(--color-text);
  }

  .option-name {
    flex: 1;
    overflow: hidden;
    color: var(--color-label-secondary, var(--color-text));
    font-size: var(--text-body);
    font-weight: 450;
    line-height: 18px;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color var(--motion-instant) var(--ease-smooth);
  }

  .offer .option-name {
    color: var(--color-muted);
  }

  .option[aria-selected="true"] .option-name,
  .option[data-active] .option-name {
    color: var(--color-text);
  }

  .option[aria-selected="true"] .option-name {
    font-weight: 550;
  }

  .aside {
    flex: none;
    color: var(--color-faint);
    font-size: var(--text-caption);
    line-height: 14px;
    font-variant-numeric: tabular-nums;
  }

  .row-tier {
    display: grid;
    flex: none;
    place-items: center;
    color: var(--color-faint);
    transition: color var(--motion-fast) var(--ease-out);
  }

  .option[data-active] .row-tier {
    color: var(--color-muted);
  }

  .offer .check {
    opacity: 0.5;
    filter: grayscale(1);
    transition:
      opacity var(--motion-fast) var(--ease-out),
      filter var(--motion-fast) var(--ease-out);
  }

  .offer[data-active] .check {
    opacity: 1;
    filter: none;
  }

  .action {
    transition: color var(--motion-fast) var(--ease-out);
  }

  .offer[data-active] .action {
    color: var(--color-text);
  }

  .offer[data-refused] .action {
    color: var(--color-danger);
  }

  .empty,
  .note {
    margin: 0;
    padding: 10px 16px;
    color: var(--color-faint);
    font-size: var(--text-label);
    line-height: 16px;
  }

  .note {
    flex: none;
  }

  .failure {
    color: var(--color-danger);
  }

  .foot {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 2px 8px 8px;
  }

  .link {
    block-size: 26px;
    padding-inline: 8px;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--color-faint);
    font: inherit;
    font-size: var(--text-label);
    cursor: default;
    transition:
      color var(--motion-fast) var(--ease-out),
      background-color var(--motion-fast) var(--ease-out);
  }

  .link:hover {
    color: var(--color-text);
  }

  .link:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: -1px;
  }

  .type {
    padding-inline-end: 8px;
    color: var(--color-faint);
    font-size: var(--text-caption);
    opacity: 0.8;
  }

  .sr {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /* stylelint-disable-next-line keyframes-name-pattern -- Svelte's global prefix. */
  @keyframes -global-model-menu-in {
    from {
      transform: scale(0.94) translateY(6px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.ui-menu.model-menu),
    :global(.ui-menu.model-menu[data-state="closed"]) {
      animation: ui-menu-fade var(--motion-instant) linear;
    }

    .find,
    .plate[data-glide],
    .dot {
      transition: none;
    }
  }

  @media (forced-colors: active) {
    .plate {
      outline: 1px solid Highlight;
    }
  }
</style>
