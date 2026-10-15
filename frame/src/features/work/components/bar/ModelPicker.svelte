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
    keyedProviders,
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
  import { Cancel01Icon, Key01Icon, Search01Icon, Tick02Icon } from "../../lib/icons";
  import * as m from "$shared/i18n/messages";
  import { nearestEffort, stepEffort } from "./ModelEffort.svelte";
  import ModelAsk from "./ModelAsk.svelte";
  import ModelGauge from "./ModelGauge.svelte";
  import ModelHero, { type HeroSubject } from "./ModelHero.svelte";
  import ModelRail, { type Seat } from "./ModelRail.svelte";
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
  const tierOf = $derived(tiers(groups.ready));

  type Item = { id: string; entry: WorkModelEntry };
  type Section = { key: string; label: string; provider: WorkModelProvider; items: Item[] };

  const nameOf = (provider: WorkModelProvider) =>
    provider === "cloud"
      ? m.ai_cloud_title()
      : provider === "compatible"
        ? m.ai_custom_name()
        : providerName(provider);

  /** Every provider Settings offers: the ready ones first, then each with the one step that readies it. */
  const seats = $derived.by<Seat[]>(() => {
    const ready: Seat[] = [];
    const rest: Seat[] = [];
    for (const provider of ["cloud", ...keyedProviders] as const) {
      if (usable(models, provider)) ready.push({ provider, standing: "ready" });
      else if (provider === "cloud") {
        if (account?.signed_in) ready.push({ provider, standing: "ready" });
        else if (account?.available && account.hosted_models === true)
          rest.push({ provider, standing: "sign_in" });
      } else
        rest.push({
          provider,
          standing:
            provider === "compatible" &&
            !models?.providers.find((row) => row.provider === provider)?.base
              ? "server"
              : keyState(models, provider) === "invalid"
                ? "refused"
                : "key",
        });
    }
    return [...ready, ...rest];
  });

  const allKeyed = $derived(seats.every((row) => row.standing === "ready"));
  let picked = $state<WorkModelProvider | null>(null);
  const shown = $derived(
    seats.find((seat) => seat.provider === picked)?.provider ??
      seats.find((seat) => seat.provider === current?.model.provider)?.provider ??
      seats[0]?.provider ??
      null,
  );
  const seat = $derived(seats.find((row) => row.provider === shown) ?? null);

  let query = $state("");
  const searching = $derived(query.trim().length > 0);
  const sections = $derived.by<Section[]>(() => {
    const matched = searching
      ? new Set(searchEntries(groups.ready, query).map((entry) => entry.id))
      : null;
    return groups.ready
      .filter((group) => searching || group.provider === shown)
      .map((group) => ({
        key: group.provider,
        label: nameOf(group.provider),
        provider: group.provider,
        items: group.entries
          .filter((entry) => !matched || matched.has(entry.id))
          .map((entry) => ({ id: entry.id, entry })),
      }))
      .filter((section) => section.items.length);
  });
  const hits = $derived(searching ? new Set(sections.map((section) => section.provider)) : null);
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
    const entry = focus?.entry ?? current;
    if (!entry) return null;
    const own = effortLevels(models, entry.id);
    return {
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
    picked = null;
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

  /** The step a provider waits on: signing in for the hosted models, otherwise Settings. */
  function step(row: Seat | null) {
    if (row?.standing === "sign_in") {
      open = false;
      reset();
      void commands.runCommand("settings.account");
    } else settings();
  }

  async function pick(provider: WorkModelProvider) {
    picked = provider;
    query = "";
    await tick();
    active = items.find((item) => item.id === chosen)?.id ?? items[0]?.id ?? null;
    reveal(active);
  }

  function reveal(id: string | null) {
    if (!id) return;
    list?.querySelector(`[data-id="${CSS.escape(id)}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function walk(to: number) {
    const next = items[Math.max(0, Math.min(items.length - 1, to))];
    if (!next) return;
    active = next.id;
    reveal(next.id);
  }

  function retyped() {
    active = items[0]?.id ?? null;
    void tick().then(() => reveal(active));
  }

  function keys(event: KeyboardEvent) {
    if (event.defaultPrevented || event.isComposing) return;
    const target = event.target as HTMLElement;
    const inField = target === field;
    const onButton = target instanceof HTMLButtonElement;
    if (
      !inField &&
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
    if (target.getAttribute("role") === "tab" && event.key.startsWith("Arrow")) return;
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
        if (query || !hero?.effort) return;
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
        event.preventDefault();
        if (item) void choose(item.entry);
        else if (!searching && seat && seat.standing !== "ready") step(seat);
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
    if (item) void choose(item.entry);
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
        {#if effort && levels.length > 1}<ModelGauge {levels} level={effort} />{/if}
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
        <ModelRail {seats} current={shown} {hits} panel="{uid}-pane" onpick={(p) => void pick(p)} />
        <div
          class="main"
          id="{uid}-pane"
          role="tabpanel"
          aria-label={searching ? m.model_search() : shown ? nameOf(shown) : m.work_model_label()}
        >
          <div class="find">
            <label class="find-row">
              <span class="find-icon" aria-hidden="true"
                ><Icon icon={Search01Icon} size={14} strokeWidth={2} /></span
              >
              <input
                bind:this={field}
                bind:value={query}
                type="text"
                role="combobox"
                spellcheck="false"
                autocomplete="off"
                placeholder={m.model_search()}
                aria-label={m.model_search()}
                aria-expanded="true"
                aria-controls="{uid}-list"
                aria-autocomplete="list"
                aria-activedescendant={active && items.some((item) => item.id === active)
                  ? slot(active)
                  : undefined}
                oninput={retyped}
              />
              {#if searching}<button
                  type="button"
                  class="find-clear"
                  aria-label={m.model_search_clear()}
                  onpointerdown={(event) => event.preventDefault()}
                  onclick={() => {
                    query = "";
                    active = home;
                    field?.focus();
                  }}><Icon icon={Cancel01Icon} size={11} strokeWidth={2.4} /></button
                >{/if}
            </label>
          </div>
          {#if hero}<ModelHero subject={hero} onEffort={(level) => void setEffort(level)} />{/if}
          {#if seat && seat.standing !== "ready" && !searching}
            <ModelAsk
              provider={seat.provider}
              name={nameOf(seat.provider)}
              standing={seat.standing}
              onact={() => step(seat)}
            />
          {:else}
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
                    {#if searching}<span class="label-mark" aria-hidden="true"
                        ><ProviderMark provider={section.provider} size={12} /></span
                      >{/if}<span>{section.label}</span>
                  </div>
                  {#each section.items as item (item.id)}
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
                        {#if small}<span class="aside"
                            >{contextSize(item.entry.context_window)}</span
                          >{/if}
                        {@render tierMark(tierOf[item.entry.id])}
                      {/if}
                    </div>
                  {/each}
                </div>
              {/each}
              {#if searching && !items.length}<p class="empty">
                  {m.model_no_match({ query: query.trim() })}
                </p>{:else if !searching && !items.length}<p class="empty">
                  {m.model_empty()}
                </p>{/if}
            </div>
          {/if}
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
          </div>
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

  :global(.ui-menu.model-menu) {
    display: flex;
    flex-direction: row;
    inline-size: 476px;
    block-size: min(468px, var(--bits-floating-available-height, 468px));
    max-block-size: none;
    padding: 0;
    transform-origin: var(--bits-floating-transform-origin, bottom right);
    animation:
      ui-menu-fade var(--motion-fast) var(--ease-out),
      model-menu-in var(--motion-page) var(--ease-spring);
  }

  :global(.ui-menu.model-menu[data-state="closed"]) {
    animation: ui-menu-out var(--motion-instant) var(--ease-exit) forwards;
  }

  .main {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-inline-size: 0;
    min-block-size: 0;
  }

  .find {
    flex: none;
    margin: 6px 6px 0;
  }

  .find-row {
    display: flex;
    align-items: center;
    gap: 8px;
    block-size: 34px;
    box-sizing: border-box;
    padding-inline: 11px 6px;
    border-radius: var(--radius-control);
    background: var(--color-field);
    color: var(--color-faint);
    transition:
      background-color var(--motion-fast) var(--ease-out),
      box-shadow var(--motion-fast) var(--ease-out);
  }

  .find-row:hover {
    background: var(--color-field-hover);
  }

  .find-row:focus-within {
    background: var(--color-field-hover);
    box-shadow: inset 0 0 0 1px var(--color-field-ring);
    color: var(--color-muted);
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

  .find input::placeholder {
    color: var(--color-faint);
    opacity: 1;
  }

  .find-clear {
    display: grid;
    flex: none;
    inline-size: 20px;
    block-size: 20px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--row-active);
    color: var(--color-muted);
    cursor: default;
    place-items: center;
    animation: find-clear-in var(--motion-fast) var(--ease-out);
  }

  .find-clear:hover {
    background: var(--row-pressed);
    color: var(--color-text);
  }

  .find-clear:focus-visible {
    outline: 2px solid var(--color-ring);
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

  @keyframes find-clear-in {
    from {
      opacity: 0;
      scale: 0.7;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.ui-menu.model-menu),
    :global(.ui-menu.model-menu[data-state="closed"]) {
      animation: ui-menu-fade var(--motion-instant) linear;
    }

    .find-row,
    .plate[data-glide] {
      transition: none;
    }

    .find-clear {
      animation: none;
    }
  }

  @media (forced-colors: active) {
    .plate {
      outline: 1px solid Highlight;
    }
  }
</style>
