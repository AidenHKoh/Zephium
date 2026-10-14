<script lang="ts">
  import { Popover } from "bits-ui";
  import { onMount, tick } from "svelte";
  import CheckmarkBadge01Icon from "@hugeicons/core-free-icons/CheckmarkBadge01Icon";
  import EyeIcon from "@hugeicons/core-free-icons/EyeIcon";
  import { accountSession } from "$domain/account";
  import type { WorkModelEntry, WorkModelProvider, WorkModelReasoning } from "$shared/ipc/bindings";
  import { commands } from "$shared/ipc/bindings";
  import {
    ModelsSession,
    contextSize,
    dollars,
    effortBars,
    effortLevels,
    effortName,
    entryOf,
    keyState,
    locked,
    pickerGroups,
    priceTier,
    providerName,
    searchEntries,
    shortName,
    usable,
  } from "$domain/ai";
  import Icon from "$shared/ui/Icon";
  import ProviderMark from "$shared/ui/ProviderMark";
  import {
    ArrowDown01Icon,
    ArrowRight01Icon,
    GlobalIcon,
    Key01Icon,
    Search01Icon,
  } from "../../lib/icons";
  import * as m from "$shared/i18n/messages";
  import ModelEffort from "./ModelEffort.svelte";
  import "$shared/ui/Menu/popover.css";

  let {
    profile,
    onsettings = () => void commands.runCommand("settings.ai"),
  }: {
    profile: string;
    /** Opens Settings → AI, where keys are added. */
    onsettings?: () => void;
  } = $props();

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
  let list = $state<HTMLElement>();
  let rail = $state<HTMLElement>();
  let field = $state<HTMLInputElement>();
  let chip = $state<HTMLButtonElement>();

  const models = $derived(session?.models ?? null);
  const current = $derived(entryOf(models, models?.effective.lead));
  const chosen = $derived(models?.effective.lead ?? null);
  const groups = $derived(pickerGroups(models, "lead", session?.endpoint ?? []));
  const allKeyed = $derived(groups.needsKey.length === 0);

  type Tile = { provider: WorkModelProvider; offer: "ready" | "key" | "refused" | "sign_in" };
  const tiles = $derived<Tile[]>([
    ...(hostedOffer ? [{ provider: "cloud" as const, offer: "sign_in" as const }] : []),
    ...groups.ready.map((group) => ({ provider: group.provider, offer: "ready" as const })),
    ...groups.needsKey.map((provider) => ({
      provider,
      offer: keyState(models, provider) === "invalid" ? ("refused" as const) : ("key" as const),
    })),
  ]);

  /** The provider the list shows; the running model's until another is picked. */
  let picked = $state<WorkModelProvider | null>(null);
  const shown = $derived(
    tiles.find((tile) => tile.provider === picked) ??
      tiles.find((tile) => tile.provider === current?.model.provider) ??
      tiles.find((tile) => tile.offer === "ready") ??
      tiles[0] ??
      null,
  );
  let query = $state("");
  const searching = $derived(query.trim().length > 0);
  const rows = $derived(
    searching
      ? searchEntries(groups.ready, query)
      : (groups.ready.find((group) => group.provider === shown?.provider)?.entries ?? []),
  );

  /** The lead's effort as Rust last said, or the one on its way there. */
  let wanted = $state<WorkModelReasoning | null>(null);
  const levels = $derived(effortLevels(models, chosen));
  const effort = $derived<WorkModelReasoning | null>(
    levels.length ? (wanted && levels.includes(wanted) ? wanted : (models?.effort ?? null)) : null,
  );
  let tray = $state(false);

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

  async function opened(next: boolean) {
    open = next;
    if (!next) {
      tray = false;
      query = "";
      picked = null;
      return;
    }
    // The person's own server says which models it serves; the rest is the curated list.
    if (usable(models, "compatible")) void session?.listEndpoint();
    await tick();
    (
      options().find((row) => row.getAttribute("aria-selected") === "true") ??
      options()[0] ??
      field
    )?.focus();
  }

  /** The menu stays while the choice lands, so the mark moves before it closes. */
  async function choose(entry: WorkModelEntry) {
    if (locked(models, entry.id) || session?.busy) return;
    if (entry.id !== chosen && !(await session?.choose("lead", entry.id))) return;
    open = false;
    tray = false;
    query = "";
    picked = null;
  }

  /** Each change goes to Rust in turn; a newer one waits for the last to land. */
  let sending = false;
  async function setEffort(level: WorkModelReasoning) {
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
    onsettings();
  }

  function signIn() {
    open = false;
    void commands.runCommand("settings.account");
  }

  function options() {
    return [...(list?.querySelectorAll<HTMLElement>("[role=option]") ?? [])];
  }

  function pick(provider: WorkModelProvider) {
    picked = provider;
    query = "";
    tray = false;
  }

  function offer(tile: Tile) {
    if (tile.offer === "sign_in") signIn();
    else if (tile.offer !== "ready") settings();
    else void tick().then(() => options()[0]?.focus());
  }

  function railKeys(event: KeyboardEvent) {
    const all = [...(rail?.querySelectorAll<HTMLElement>("[role=tab]") ?? [])];
    const at = all.indexOf(document.activeElement as HTMLElement);
    const step = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[event.key];
    let next: number | null = null;
    if (step !== undefined) next = Math.max(0, Math.min(all.length - 1, at + step));
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = all.length - 1;
    if (next === null) return;
    event.preventDefault();
    const tile = tiles[next];
    if (tile) pick(tile.provider);
    all[next]?.focus();
  }

  function listKeys(event: KeyboardEvent) {
    const all = options();
    const at = all.indexOf(document.activeElement as HTMLElement);
    if (at === -1) return;
    const vertical = { ArrowDown: 1, ArrowUp: -1 }[event.key];
    if (vertical !== undefined) {
      event.preventDefault();
      all[Math.max(0, Math.min(all.length - 1, at + vertical))]?.focus();
      return;
    }
    if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      all[event.key === "Home" ? 0 : all.length - 1]?.focus();
      return;
    }
    if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && !searching) {
      const ready = tiles.filter((tile) => tile.offer === "ready");
      const here = ready.findIndex((tile) => tile.provider === shown?.provider);
      const next = ready[here + (event.key === "ArrowRight" ? 1 : -1)];
      if (!next) return;
      event.preventDefault();
      pick(next.provider);
      void tick().then(() => options()[0]?.focus());
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      const entry = rows[at];
      if (entry) void choose(entry);
      return;
    }
    // Typing anywhere in the list searches.
    if (event.key.length === 1 && !event.metaKey && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      query += event.key;
      field?.focus();
    }
  }

  function clicked(event: MouseEvent) {
    const option = (event.target as Element).closest<HTMLElement>("[role=option]");
    const entry = option ? rows[Number(option.dataset.index)] : undefined;
    if (entry) void choose(entry);
  }

  function fieldKeys(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      options()[0]?.focus();
    } else if (event.key === "Enter") {
      event.preventDefault();
      const first = rows[0];
      if (first) void choose(first);
    }
  }

  function escape(event: KeyboardEvent) {
    if (tray) {
      event.preventDefault();
      tray = false;
      chip?.focus();
    } else if (query) {
      event.preventDefault();
      query = "";
      field?.focus();
    }
  }

  async function toggleTray() {
    tray = !tray;
    if (!tray) return;
    await tick();
    list?.parentElement?.querySelector<HTMLElement>("[role=slider]")?.focus();
  }

  const triggerLabel = $derived(
    current
      ? effort
        ? m.model_trigger_effort({ model: current.display_name, effort: effortName(effort) })
        : m.work_model_trigger({ model: current.display_name })
      : m.work_model_label(),
  );
</script>

{#snippet meta(entry: WorkModelEntry)}
  {@const tier = priceTier(entry.price)}
  <span class="meta">
    {#if entry.supports.vision}<span class="glyph" title={m.model_vision()}
        ><Icon icon={EyeIcon} size={11} strokeWidth={1.8} /><span class="sr"
          >{m.model_vision()}</span
        ></span
      >{/if}
    {#if entry.supports.native_search}<span class="glyph" title={m.model_web()}
        ><Icon icon={GlobalIcon} size={11} strokeWidth={1.8} /><span class="sr"
          >{m.model_web()}</span
        ></span
      >{/if}
    <span class="context" title={m.model_context({ size: contextSize(entry.context_window) })}
      >{contextSize(entry.context_window)}</span
    >
    {#if tier && entry.price}<span class="price"
        ><span class="tier" aria-label={m.model_tier({ tier })}
          ><span class="paid">{"$".repeat(tier)}</span><span class="unpaid"
            >{"$".repeat(4 - tier)}</span
          ></span
        ><span class="exact"
          >{m.model_price_short({
            input: dollars(entry.price.input),
            output: dollars(entry.price.output),
          })}</span
        ></span
      >{/if}
  </span>
{/snippet}

{#snippet row(entry: WorkModelEntry, index: number)}
  {@const selected = entry.id === chosen}
  {@const closed = locked(models, entry.id)}
  {@const efforts = selected ? levels : []}
  <li class="row" data-selected={selected ? "" : undefined}>
    <div
      class="option"
      role="option"
      data-index={index}
      tabindex={selected || (index === 0 && !rows.some((item) => item.id === chosen)) ? 0 : -1}
      aria-selected={selected}
      aria-disabled={closed || !!session?.busy || undefined}
    >
      {#if searching}<span class="row-mark" aria-hidden="true"
          ><ProviderMark provider={entry.model.provider} size={14} /></span
        >{/if}
      <span class="words">
        <span class="name"
          >{entry.display_name}{#if entry.recommended}<span
              class="recommended"
              title={m.model_recommended()}
              ><Icon icon={CheckmarkBadge01Icon} size={12} strokeWidth={1.8} /><span class="sr"
                >{m.model_recommended()}</span
              ></span
            >{/if}</span
        >
        {#if closed}<span class="meta locked">{m.work_model_locked()}</span>{:else}{@render meta(
            entry,
          )}{/if}
      </span>
      <span class="radio" aria-hidden="true"></span>
    </div>
    {#if efforts.length && effort}<button
        type="button"
        class="chip"
        bind:this={chip}
        aria-expanded={tray}
        aria-label={m.model_effort_choose({ level: effortName(effort) })}
        onclick={() => void toggleTray()}
        ><span>{effortName(effort)}</span><span class="chip-chevron" aria-hidden="true"
          ><Icon icon={ArrowRight01Icon} size={10} strokeWidth={2.2} /></span
        ></button
      >{/if}
  </li>
{/snippet}

{#snippet bars(level: WorkModelReasoning)}
  <span class="bars" data-level={level} aria-hidden="true"
    >{#each [1, 2, 3] as bar (bar)}<span class="bar" class:on={bar <= effortBars(level)}
      ></span>{/each}</span
  >
{/snippet}

{#if keyless}
  <button type="button" class="model-trigger add-key" onclick={settings}>
    <span class="trigger-mark" aria-hidden="true"><Icon icon={Key01Icon} size={14} /></span>
    <span class="trigger-name"
      >{hostedOffer ? m.work_model_start() : m.work_model_add_model_key()}</span
    >
  </button>
{:else}<Popover.Root {open} onOpenChange={(next) => void opened(next)}>
    <Popover.Trigger class="model-trigger" aria-label={triggerLabel} disabled={!models}>
      {#if current}
        <span class="trigger-mark" aria-hidden="true"
          ><ProviderMark provider={current.model.provider} size={15} /></span
        >
        <span class="trigger-name">{shortName(current)}</span>
        {#if effort}{@render bars(effort)}{/if}
      {:else}
        <span class="trigger-name empty">{m.work_model_none()}</span>
      {/if}
      <span class="trigger-chevron" aria-hidden="true"
        ><Icon icon={ArrowDown01Icon} size={11} strokeWidth={2} /></span
      >
    </Popover.Trigger>
    <Popover.Portal>
      <Popover.Content
        class="ui-menu model-menu"
        side="top"
        align="end"
        sideOffset={10}
        collisionPadding={12}
        aria-label={m.work_model_label()}
        onOpenAutoFocus={(event) => event.preventDefault()}
        onEscapeKeydown={escape}
      >
        <div
          class="rail"
          role="tablist"
          aria-orientation="vertical"
          aria-label={m.model_providers()}
          tabindex="-1"
          bind:this={rail}
          onkeydown={railKeys}
        >
          {#each tiles as tile (tile.provider)}
            {@const lit = !searching && shown?.provider === tile.provider}
            <button
              type="button"
              class="tile"
              role="tab"
              data-offer={tile.offer}
              aria-selected={lit}
              tabindex={lit || (!shown && tile === tiles[0]) ? 0 : -1}
              aria-label={tile.provider === "cloud"
                ? m.ai_cloud_title()
                : providerName(tile.provider)}
              title={tile.provider === "cloud" ? m.ai_cloud_title() : providerName(tile.provider)}
              onclick={() => {
                pick(tile.provider);
                if (tile.offer === "ready") offer(tile);
              }}
            >
              <span class="tile-mark"><ProviderMark provider={tile.provider} size={18} /></span>
              {#if tile.offer === "key" || tile.offer === "refused"}<span
                  class="badge"
                  data-refused={tile.offer === "refused" ? "" : undefined}
                  aria-hidden="true"><Icon icon={Key01Icon} size={8} strokeWidth={2.4} /></span
                >{/if}
            </button>
          {/each}
        </div>
        <div class="pane">
          <div class="head">
            <span class="caption">{m.model_caption()}</span>
            <label class="search">
              <span class="search-icon" aria-hidden="true"
                ><Icon icon={Search01Icon} size={12} strokeWidth={2} /></span
              >
              <input
                bind:this={field}
                bind:value={query}
                type="text"
                spellcheck="false"
                autocomplete="off"
                placeholder={m.model_search_placeholder()}
                aria-label={m.model_search()}
                onkeydown={fieldKeys}
              />
            </label>
          </div>
          <div class="body" bind:this={list}>
            {#if searching}
              {#if rows.length}
                <ul
                  class="options"
                  role="listbox"
                  aria-label={m.model_search()}
                  onkeydown={listKeys}
                  onclick={clicked}
                >
                  {#each rows as entry, index (entry.id)}{@render row(entry, index)}{/each}
                </ul>
              {:else}
                <p class="empty">{m.model_no_match({ query: query.trim() })}</p>
              {/if}
            {:else if shown?.offer === "ready"}
              <div class="heading" role="presentation">{providerName(shown.provider)}</div>
              <ul
                class="options"
                role="listbox"
                aria-label={providerName(shown.provider)}
                onkeydown={listKeys}
                onclick={clicked}
              >
                {#each rows as entry, index (entry.id)}{@render row(entry, index)}{/each}
              </ul>
            {:else if shown}
              {@const name =
                shown.provider === "cloud" ? m.ai_cloud_title() : providerName(shown.provider)}
              <div class="offer" role="tabpanel" aria-label={name}>
                <span class="offer-mark" aria-hidden="true"
                  ><ProviderMark provider={shown.provider} size={26} /></span
                >
                <span class="offer-name">{name}</span>
                <p class="offer-text">
                  {shown.offer === "sign_in"
                    ? m.model_hosted()
                    : shown.offer === "refused"
                      ? m.model_key_refused({ provider: name })
                      : m.model_key_needed({ provider: name })}
                </p>
                <button
                  type="button"
                  class="offer-action"
                  onclick={() => (shown.offer === "sign_in" ? signIn() : settings())}
                  >{shown.offer === "sign_in" ? m.work_model_sign_in() : m.model_add_key()}</button
                >
              </div>
            {/if}
          </div>
          {#if tray && effort && levels.length && !searching}<div class="tray">
              <ModelEffort
                {levels}
                value={effort}
                onchange={(level) => void setEffort(level)}
                onclose={() => {
                  tray = false;
                  chip?.focus();
                }}
              />
            </div>{/if}
          {#if searchOff}<p class="note">{m.work_model_no_search()}</p>{/if}
          {#if session?.fault?.action === "choose:lead"}<p class="note failure" role="alert">
              {m.work_model_failed()}
            </p>{/if}
          {#if session?.fault?.action === "effort"}<p class="note failure" role="alert">
              {m.model_effort_failed()}
            </p>{/if}
          <div class="foot">
            <button type="button" class="ui-menu-item manage" onclick={settings}>
              <span class="ui-menu-icon" aria-hidden="true"
                ><Icon icon={Key01Icon} size={14} /></span
              >
              <span class="manage-name"
                >{allKeyed ? m.work_model_manage() : m.work_model_add_key()}</span
              >
            </button>
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
    padding-inline: 8px 6px;
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

  .trigger-mark,
  .trigger-chevron {
    display: grid;
    flex: none;
    place-items: center;
  }

  .trigger-chevron {
    color: var(--color-faint);
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

  /* Three hairlines rising to the level the lead thinks at. */
  .bars {
    display: inline-flex;
    flex: none;
    align-items: flex-end;
    gap: 1.5px;
    block-size: 10px;
    margin-inline-start: 1px;
  }

  .bar {
    inline-size: 1.5px;
    border-radius: 1px;
    background: currentcolor;
    opacity: 0.28;
    transition: opacity var(--motion-base) var(--ease-out);
  }

  .bar:nth-child(1) {
    block-size: 4px;
  }

  .bar:nth-child(2) {
    block-size: 7px;
  }

  .bar:nth-child(3) {
    block-size: 10px;
  }

  .bar.on {
    opacity: 0.9;
  }

  .bars[data-level="xhigh"] .bar.on,
  .bars[data-level="max"] .bar.on {
    opacity: 1;
  }

  .bars[data-level="max"] .bar.on {
    background: color-mix(in oklab, var(--color-agent-lead-sheen) 70%, currentcolor);
  }

  /* Over the canvas the menu stands on the solid floating material, like
     every Work menu. */
  :global(.model-menu) {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr);
    inline-size: 372px;
    min-block-size: min(272px, var(--bits-floating-available-height, 272px));
    max-block-size: min(480px, var(--bits-floating-available-height, 480px));
    padding: 0;
    background: var(--color-float);
    backdrop-filter: none;
    transform-origin: var(--bits-floating-transform-origin, bottom right);
  }

  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    min-block-size: 0;
    padding: 8px 0;
    overflow-y: auto;
    border-inline-end: 1px solid var(--color-menu-separator);
    outline: none;
    scrollbar-width: none;
  }

  .tile {
    position: relative;
    display: grid;
    flex: none;
    inline-size: 32px;
    block-size: 32px;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--color-text);
    cursor: default;
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      box-shadow var(--motion-instant) var(--ease-smooth);
  }

  .tile:hover {
    background: var(--row-hover);
  }

  .tile[aria-selected="true"] {
    background: var(--row-active);
    box-shadow: var(--row-rim);
  }

  .tile:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: -1px;
  }

  .tile-mark {
    display: grid;
    place-items: center;
    transition:
      opacity var(--motion-fast) var(--ease-out),
      filter var(--motion-fast) var(--ease-out);
  }

  .tile[data-offer="key"] .tile-mark,
  .tile[data-offer="refused"] .tile-mark,
  .tile[data-offer="sign_in"] .tile-mark {
    opacity: 0.42;
    filter: grayscale(1);
  }

  .tile:hover .tile-mark,
  .tile[aria-selected="true"] .tile-mark {
    opacity: 1;
    filter: none;
  }

  .badge {
    position: absolute;
    inset-block-end: 3px;
    inset-inline-end: 3px;
    display: grid;
    inline-size: 12px;
    block-size: 12px;
    place-items: center;
    border-radius: 50%;
    background: var(--color-float);
    box-shadow: 0 0 0 1px var(--color-menu-separator);
    color: var(--color-muted);
  }

  .badge[data-refused] {
    color: var(--color-danger);
  }

  .pane {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    min-inline-size: 0;
  }

  .head {
    display: flex;
    flex: none;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 6px 12px;
  }

  .caption,
  .heading {
    color: var(--color-faint);
    font-size: var(--text-caption);
    font-weight: 550;
    line-height: 14px;
    letter-spacing: 0.01em;
  }

  .caption {
    flex: none;
  }

  .search {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 6px;
    min-inline-size: 0;
    block-size: 26px;
    padding-inline: 8px;
    border-radius: var(--radius-field);
    background: var(--color-field);
    color: var(--color-faint);
    transition: background-color var(--motion-instant) var(--ease-smooth);
  }

  .search:hover {
    background: var(--color-field-hover);
  }

  .search:focus-within {
    box-shadow: 0 0 0 2px var(--color-field-ring);
    color: var(--color-muted);
  }

  .search-icon {
    display: grid;
    flex: none;
    place-items: center;
  }

  .search input {
    flex: 1;
    min-inline-size: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-label);
    outline: none;
  }

  .search input::placeholder {
    color: var(--color-faint);
  }

  .body {
    flex: 1;
    min-block-size: 0;
    padding: 0 6px 6px;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .heading {
    padding: 6px 8px 4px;
  }

  .options {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    position: relative;
  }

  .option {
    display: flex;
    align-items: center;
    gap: 10px;
    min-block-size: 42px;
    padding: 5px 34px 5px 8px;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    outline: none;
    cursor: default;
    transition: background-color var(--motion-instant) var(--ease-smooth);
  }

  .option:hover,
  .option:focus-visible {
    background: var(--row-active);
  }

  .option:focus-visible {
    box-shadow: inset 0 0 0 1px var(--color-ring);
  }

  .option[aria-disabled="true"] {
    opacity: 0.45;
  }

  .row[data-selected] .option {
    padding-inline-end: 112px;
  }

  .row-mark {
    display: grid;
    flex: none;
    inline-size: 16px;
    place-items: center;
  }

  .words {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-inline-size: 0;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--color-text);
    font-size: var(--text-body);
    font-weight: 500;
    line-height: 17px;
    letter-spacing: -0.003em;
    overflow-wrap: anywhere;
  }

  .recommended {
    display: inline-grid;
    flex: none;
    place-items: center;
    color: var(--color-faint);
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--color-faint);
    font-size: var(--text-caption);
    line-height: 14px;
    font-variant-numeric: tabular-nums;
  }

  .meta.locked {
    color: var(--color-muted);
  }

  .glyph {
    display: inline-grid;
    place-items: center;
  }

  .price {
    position: relative;
    display: inline-flex;
  }

  .tier {
    letter-spacing: 0.04em;
    transition: opacity var(--motion-fast) var(--ease-out);
  }

  .paid {
    color: var(--color-muted);
  }

  .unpaid {
    opacity: 0.45;
  }

  /* Pointing at a model trades its price tier for its price. */
  .exact {
    position: absolute;
    inset-inline-start: 0;
    white-space: nowrap;
    opacity: 0;
    transition: opacity var(--motion-fast) var(--ease-out);
  }

  .option:hover .tier,
  .option:focus-visible .tier {
    opacity: 0;
  }

  .option:hover .exact,
  .option:focus-visible .exact {
    opacity: 1;
  }

  .radio {
    position: absolute;
    inset-inline-end: 12px;
    inset-block-start: 50%;
    inline-size: 14px;
    block-size: 14px;
    box-sizing: border-box;
    border: 1px solid var(--color-border-strong);
    border-radius: 50%;
    transform: translateY(-50%);
    transition:
      border-color var(--motion-fast) var(--ease-out),
      background-color var(--motion-fast) var(--ease-out);
  }

  .row[data-selected] .radio {
    border: 4px solid var(--color-text);
    background: var(--color-float);
  }

  .chip {
    position: absolute;
    inset-inline-end: 34px;
    inset-block-start: 50%;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    block-size: 22px;
    padding-inline: 8px 5px;
    border: 0;
    border-radius: var(--radius-capsule);
    background: var(--color-fill);
    color: var(--color-muted);
    font: inherit;
    font-size: var(--text-caption);
    font-weight: 550;
    cursor: default;
    transform: translateY(-50%);
    transition:
      background-color var(--motion-instant) var(--ease-smooth),
      color var(--motion-instant) var(--ease-smooth);
  }

  .chip:hover,
  .chip[aria-expanded="true"] {
    background: var(--color-fill-hover);
    color: var(--color-text);
  }

  .chip:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: 1px;
  }

  .chip-chevron {
    display: grid;
    place-items: center;
    transition: transform var(--motion-base) var(--ease-out);
  }

  .chip[aria-expanded="true"] .chip-chevron {
    transform: rotate(90deg);
  }

  .chip:dir(rtl) .chip-chevron {
    transform: scaleX(-1);
  }

  .chip:dir(rtl)[aria-expanded="true"] .chip-chevron {
    transform: rotate(90deg);
  }

  .tray {
    flex: none;
    margin: 0 6px 6px;
    border-radius: calc(var(--radius-menu) - var(--menu-pad));
    background: var(--color-fill);
  }

  .empty,
  .note {
    margin: 0;
    padding: 10px 12px;
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

  .offer {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 14px 10px 10px;
  }

  .offer-mark {
    display: grid;
    inline-size: 40px;
    block-size: 40px;
    place-items: center;
    border-radius: var(--radius-card);
    background: var(--color-fill);
  }

  .offer-name {
    margin-block-start: 4px;
    color: var(--color-text);
    font-size: var(--text-body);
    font-weight: 550;
  }

  .offer-text {
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 16px;
  }

  .offer-action {
    margin-block-start: 6px;
    block-size: 26px;
    padding-inline: 12px;
    border: 0;
    border-radius: var(--radius-control);
    background: var(--color-control);
    box-shadow: var(--shadow-control);
    color: var(--color-on-control);
    font: inherit;
    font-size: var(--text-label);
    font-weight: 550;
    cursor: default;
  }

  .offer-action:hover {
    background: var(--color-control-hover);
  }

  .offer-action:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: 1px;
  }

  .foot {
    flex: none;
    padding: 4px 6px 6px;
    border-block-start: 1px solid var(--color-menu-separator);
  }

  .manage {
    inline-size: 100%;
    min-block-size: 30px;
    border: 0;
    background: transparent;
    font: inherit;
    font-size: var(--text-label);
    text-align: start;
  }

  .manage:hover,
  .manage:focus-visible {
    background: var(--row-active);
  }

  .manage-name {
    color: var(--color-muted);
  }

  .manage:hover .manage-name,
  .manage:focus-visible .manage-name {
    color: var(--color-text);
  }

  .sr {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  @media (prefers-reduced-motion: reduce) {
    .chip-chevron,
    .bar {
      transition: none;
    }
  }

  @media (forced-colors: active) {
    .tile[aria-selected="true"],
    .option[aria-selected="true"] {
      outline: 1px solid Highlight;
    }
  }
</style>
