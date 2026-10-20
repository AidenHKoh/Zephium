<script lang="ts">
  import { onMount } from "svelte";
  import {
    accountSession as session,
    creditsCall,
    creditsTotal,
    requestStanding,
  } from "$domain/account";
  import type { AccountFaultV1, AccountRequestV1 } from "$shared/ipc/bindings";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import { explainAccountFault } from "../../lib/account";
  import { groupCalls, type CallGroup } from "../../lib/call-groups";
  import { taskTitle } from "../../lib/task-titles";

  /**
   * Hosted usage, one row per Work task (voice and the rest have their own).
   * Which task made a call is this computer's own record; the service only
   * ever sees request ids. Pages load by the service's cursor, on request.
   */
  let requests = $state.raw<AccountRequestV1[]>([]);
  let cursor = $state<string | null>(null);
  let loading = $state(true);
  let failure = $state<AccountFaultV1 | null>(null);
  let openGroup = $state<string | null>(null);
  let open = $state<string | null>(null);
  let receipt = $state.raw<AccountRequestV1 | AccountFaultV1 | null>(null);
  let titles = $state.raw<Record<string, string | null>>({});
  /** Tasks whose name was asked for; not state, so asking never re-runs the effect. */
  const asked: Record<string, boolean> = {};
  const per = $derived(session.account()?.usage?.credit_units ?? null);
  const groups = $derived(groupCalls(requests));
  const when = new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });

  async function load(after: string | null) {
    loading = true;
    const page = await session.usageHistory(after);
    loading = false;
    failure = page.fault;
    if (page.fault) return;
    requests = after ? [...requests, ...page.requests] : page.requests;
    cursor = page.next_cursor;
  }

  onMount(() => void load(null));

  $effect(() => {
    for (const group of groups) {
      if (!group.task || asked[group.key]) continue;
      asked[group.key] = true;
      const { profile, work } = group.task;
      void taskTitle(profile, work).then((title) => {
        titles = { ...titles, [group.key]: title };
        // A failed read may be retried the next time the history opens.
        if (title === null) delete asked[group.key];
      });
    }
  });

  function name(group: CallGroup) {
    if (group.kind === "voice") return m.account_history_voice();
    if (group.kind === "other") return m.account_history_other();
    return titles[group.key] ?? m.account_history_task();
  }

  function toggleGroup(key: string) {
    openGroup = openGroup === key ? null : key;
    open = null;
  }

  async function toggle(id: string) {
    if (open === id) {
      open = null;
      return;
    }
    open = id;
    receipt = null;
    const read = await session.receipt(id);
    if (open === id) receipt = read;
  }

  function time(iso: string | null) {
    const date = iso ? new Date(iso) : null;
    return date && !Number.isNaN(date.getTime()) ? when.format(date) : "";
  }

  function count(calls: number) {
    return calls === 1 ? m.account_history_call_one() : m.account_history_calls({ count: calls });
  }

  function total(group: CallGroup) {
    const amount = creditsTotal(group.total, per);
    const charged = amount ? m.account_request_charged({ amount }) : null;
    if (!group.unsettled) return charged ?? "";
    return [charged, m.account_request_running()].filter(Boolean).join(" · ");
  }

  function cost(request: AccountRequestV1) {
    switch (requestStanding(request.state)) {
      case "charged": {
        const amount = creditsCall(request.charged ?? "0", per);
        return amount ? m.account_request_charged({ amount }) : m.account_request_charged_plain();
      }
      case "free":
        return m.account_request_free();
      case "checking":
        return m.account_request_checking();
      case "running":
        return m.account_request_running();
    }
  }

  function funding(request: AccountRequestV1) {
    if (requestStanding(request.state) !== "charged") return null;
    switch (request.funding) {
      case "allowance":
        return m.account_funding_allowance();
      case "purchased":
        return m.account_funding_purchased();
      case "mixed":
        return m.account_funding_mixed();
      default:
        return null;
    }
  }

  function details(request: AccountRequestV1) {
    const held = creditsCall(request.reserved, per);
    const finished = request.finished_at
      ? m.account_receipt_finished({ time: time(request.finished_at) })
      : null;
    const paid = funding(request);
    let line: string | null;
    switch (requestStanding(request.state)) {
      case "charged": {
        const amount = creditsCall(request.charged ?? "0", per);
        line = amount ? m.account_receipt_charged({ amount }) : m.account_request_charged_plain();
        break;
      }
      case "free":
        line = m.account_receipt_free();
        break;
      case "checking":
        line = [m.account_receipt_checking(), held && m.account_receipt_held({ amount: held })]
          .filter(Boolean)
          .join(" · ");
        break;
      case "running":
        line = held ? m.account_receipt_held({ amount: held }) : m.account_request_running();
        break;
    }
    return [line, paid, finished].filter(Boolean).join(" · ");
  }
</script>

<div class="history" aria-busy={loading}>
  <h3>{m.account_history_row()}</h3>
  {#if failure && !requests.length}<p class="quiet" role="alert">
      {explainAccountFault(failure, null)}
    </p>
  {:else if loading && !requests.length}<p class="quiet">{m.account_usage_loading()}</p>
  {:else if !requests.length}<p class="quiet">{m.account_history_empty()}</p>
  {:else}
    <ul class="groups">
      {#each groups as group (group.key)}
        <li>
          <button
            type="button"
            class="group"
            aria-expanded={openGroup === group.key}
            aria-controls="calls-{group.key}"
            onclick={() => toggleGroup(group.key)}
          >
            <span class="what">
              <span class="name" data-quiet={group.task && !titles[group.key] ? "" : undefined}
                >{name(group)}</span
              >
              <span class="meta"
                >{[time(group.latest), count(group.calls.length)].filter(Boolean).join(" · ")}</span
              >
            </span>
            <span class="cost" data-standing={group.unsettled ? "running" : "charged"}
              >{total(group)}</span
            >
          </button>
          {#if openGroup === group.key}
            <ul class="calls" id="calls-{group.key}">
              {#each group.calls as request (request.id)}
                <li>
                  <button
                    type="button"
                    class="request"
                    aria-expanded={open === request.id}
                    aria-label={m.account_request_details()}
                    onclick={() => void toggle(request.id)}
                  >
                    <span class="model"
                      >{request.model_id ?? "—"}{#if funding(request)}<span class="funding"
                          >{funding(request)}</span
                        >{/if}</span
                    >
                    <span class="when">{time(request.started_at)}</span>
                    <span class="cost" data-standing={requestStanding(request.state)}
                      >{cost(request)}</span
                    >
                  </button>
                  {#if open === request.id}<p class="receipt" role="status">
                      {#if receipt === null}{m.account_usage_loading()}
                      {:else if typeof receipt === "string"}{explainAccountFault(receipt, null)}
                      {:else}{details(receipt)}{/if}
                    </p>{/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
    {#if failure}<p class="quiet" role="alert">{m.account_history_failed()}</p>{/if}
    {#if cursor}<div class="more">
        <Button size="compact" variant="ghost" pending={loading} onclick={() => void load(cursor)}
          >{m.account_history_more()}</Button
        >
      </div>{/if}
  {/if}
</div>

<style>
  .history {
    position: relative;
    padding: 14px 18px 10px;
  }

  .history::before {
    content: "";
    position: absolute;
    inset-inline: 16px 0;
    top: 0;
    height: 1px;
    background: var(--color-border);
  }

  h3 {
    margin: 0 0 8px;
    color: var(--color-text);
    font-size: var(--text-page-title);
    font-weight: 500;
    line-height: 19px;
    letter-spacing: -0.008em;
  }

  ul {
    display: grid;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .group,
  .request {
    inline-size: calc(100% + 16px);
    margin: 0 -8px;
    padding: 7px 8px;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-label);
    text-align: start;
    cursor: default;
  }

  .group {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 16px;
    padding-block: 9px;
  }

  .request {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 8.5em;
    align-items: baseline;
    gap: 16px;
  }

  .group:hover,
  .request:hover {
    background: var(--color-fill-hover);
  }

  .group[aria-expanded="true"],
  .request[aria-expanded="true"] {
    background: var(--color-fill);
  }

  .group:focus-visible,
  .request:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: -2px;
  }

  .what {
    display: grid;
    gap: 1px;
    min-inline-size: 0;
  }

  .name {
    overflow: hidden;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .name[data-quiet] {
    color: var(--color-muted);
  }

  .meta {
    color: var(--color-muted);
    font-variant-numeric: tabular-nums;
  }

  /* Calls sit in under their task, indented to read as its parts. */
  .calls {
    margin-inline-start: 12px;
    padding-block-end: 4px;
  }

  .cost {
    text-align: end;
  }

  .model {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .funding {
    margin-inline-start: 8px;
    color: var(--color-muted);
    font-size: var(--text-caption);
  }

  .when,
  .cost {
    color: var(--color-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .cost[data-standing="charged"] {
    color: var(--color-text);
  }

  .receipt,
  .quiet {
    margin: 2px 0 6px;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 1.5;
    font-variant-numeric: tabular-nums;
  }

  .more {
    display: flex;
    justify-content: flex-start;
    padding-block: 4px;
    margin-inline-start: -8px;
  }
</style>
