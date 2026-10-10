<script lang="ts">
  import Icon from "$shared/ui/Icon";
  import * as m from "$shared/i18n/messages";
  import type { ObjectActions, SubjectView } from "../../lib/board/types";
  import { siteName } from "../../lib/run/site";
  import { ArrowUpRight01Icon } from "./icons";
  import Inline from "./Inline.svelte";
  import Mark from "./Mark.svelte";
  /** One thing in a service — a pull request, an issue, a thread — read the way that service shows it. */
  let {
    object,
    actions = {},
    centre = false,
  }: { object: SubjectView; actions?: ObjectActions; centre?: boolean } = $props();
  const host = $derived.by(() => {
    try {
      return new URL(object.url).hostname.replace(/^www\./u, "");
    } catch {
      return "";
    }
  });
  const service = $derived(host ? siteName(host) : "");
  /** "Ana Kowalska" → AK; a handle stands for one person, so it gives one letter. */
  const initials = (name: string) => {
    const words = name.trim().split(/\s+/u);
    const letters = words.length > 1 ? [words[0], words.at(-1)] : [words[0]];
    return letters.map((word) => word?.[0]?.toUpperCase() ?? "").join("");
  };
  const open = (event: MouseEvent) => {
    if (!actions.link) return;
    event.preventDefault();
    actions.link(object.url);
  };
</script>

<article class="subject" class:centre aria-label={object.heading}>
  <header>
    <span class="where">
      {#if host}<Mark address={object.url} size={16} />{/if}
      <span class="noun">{service ? `${service} · ${object.noun}` : object.noun}</span>
      {#if object.reference}<span class="reference">{object.reference}</span>{/if}
    </span>
    {#if object.status}<span class="state" data-tone={object.status.tone}
        >{object.status.label}</span
      >{/if}
  </header>

  <h3>{object.heading}</h3>
  {#if object.summary}<p class="summary"><Inline text={object.summary} /></p>{/if}

  {#if object.people.length || object.facts.length}<dl class="facts">
      {#each object.people as person, index (index)}<div>
          <dt>{person.role}</dt>
          <dd class="person">
            <span class="avatar" aria-hidden="true">{initials(person.name)}</span>
            <span class="name">{person.name}</span>
          </dd>
        </div>{/each}
      {#each object.facts as fact, index (index)}<div>
          <dt>{fact.label}</dt>
          <dd>{fact.value}</dd>
        </div>{/each}
    </dl>{/if}

  <footer>
    {#if object.labels.length}<ul class="labels">
        {#each object.labels as label (label)}<li>{label}</li>{/each}
      </ul>{/if}
    <a class="open nodrag" href={object.url} onclick={open}
      >{service ? m.work_list_open_in({ app: service }) : m.work_env_open()}<Icon
        icon={ArrowUpRight01Icon}
        size={13}
      /></a
    >
  </footer>
</article>

<style>
  .subject {
    display: flex;
    flex-direction: column;
    gap: 10px;
    box-sizing: border-box;
    inline-size: 100%;
    padding: 18px 22px 16px;
    border-radius: var(--radius-card);
    background: var(--color-surface);
    box-shadow: var(--shadow-raised);
    color: var(--color-text);
  }

  .centre {
    gap: 14px;
    padding: 26px 30px 22px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-block-size: 22px;
  }

  .where {
    display: flex;
    align-items: center;
    gap: 7px;
    min-inline-size: 0;
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .noun {
    flex: none;
    font-weight: 500;
  }

  .reference {
    overflow: hidden;
    color: var(--color-faint);
    font-family: var(--font-mono);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state {
    --tone: var(--color-muted);

    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 6px;
    block-size: 22px;
    padding: 0 9px 0 8px;
    border-radius: var(--radius-capsule);
    background: color-mix(in srgb, var(--tone) 14%, transparent);
    color: var(--tone);
    font-size: var(--text-label);
    font-weight: 600;
  }

  .state::before {
    inline-size: 6px;
    block-size: 6px;
    border-radius: var(--radius-capsule);
    background: currentcolor;
    content: "";
  }

  .state[data-tone="active"] {
    --tone: var(--color-success);
  }

  .state[data-tone="done"] {
    --tone: var(--color-info);
  }

  .state[data-tone="waiting"] {
    --tone: var(--color-warning);
  }

  .state[data-tone="blocked"] {
    --tone: var(--color-danger);
  }

  h3 {
    margin: 2px 0 0;
    font-size: var(--text-object-title);
    font-weight: 650;
    line-height: 1.3;
    letter-spacing: -0.015em;
    overflow-wrap: anywhere;
    text-wrap: balance;
  }

  .centre h3 {
    font-size: var(--text-title);
  }

  .summary {
    margin: 0;
    max-inline-size: 68ch;
    color: var(--color-label-secondary);
    font-size: var(--text-body);
    line-height: 1.5;
    text-wrap: pretty;
  }

  .centre .summary {
    font-size: var(--text-reading);
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 32px;
    margin: 4px 0 0;
    padding-block-start: 14px;
    border-block-start: 1px solid var(--color-border);
  }

  .facts div {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-inline-size: 0;
    max-inline-size: 100%;
  }

  dt {
    color: var(--color-muted);
    font-size: var(--text-caption);
    font-weight: 500;
  }

  dd {
    margin: 0;
    overflow: hidden;
    font-size: var(--text-body);
    font-variant-numeric: tabular-nums;
    font-weight: 550;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .person {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .avatar {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 18px;
    block-size: 18px;
    border-radius: var(--radius-capsule);
    background: var(--color-fill-strong);
    color: var(--color-label-secondary);
    font-size: 9px;
    font-weight: 650;
    letter-spacing: 0.02em;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-block-start: 2px;
  }

  .labels {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    min-inline-size: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .labels li {
    padding: 2px 8px;
    border-radius: var(--radius-capsule);
    background: var(--color-fill);
    color: var(--color-label-secondary);
    font-size: var(--text-label);
  }

  .open {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 4px;
    margin-inline-start: auto;
    color: var(--color-muted);
    font-size: var(--text-label);
    font-weight: 500;
    text-decoration: none;
    cursor: default;
  }

  .open:hover {
    color: var(--color-text);
  }
</style>
