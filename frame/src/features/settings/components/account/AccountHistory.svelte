<script lang="ts">
  import { onMount } from "svelte";
  import { accountSession as session, credits, requestStanding } from "$domain/account";
  import type { AccountFaultV1, AccountRequestV1 } from "$shared/ipc/bindings";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import { explainAccountFault } from "../../lib/account";

  let requests = $state.raw<AccountRequestV1[]>([]);
  let cursor = $state<string | null>(null);
  let loading = $state(true);
  let failure = $state<AccountFaultV1 | null>(null);
  let open = $state<string | null>(null);
  let receipt = $state.raw<AccountRequestV1 | AccountFaultV1 | null>(null);
  const scale = $derived(session.account()?.usage?.unit_scale ?? 1_000_000);
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

  function cost(request: AccountRequestV1) {
    switch (requestStanding(request.state)) {
      case "charged":
        return m.account_request_charged({ amount: credits(request.charged ?? "0", scale) ?? "0" });
      case "free":
        return m.account_request_free();
      case "checking":
        return m.account_request_checking();
      case "running":
        return m.account_request_running();
    }
  }

  function details(request: AccountRequestV1) {
    const held = credits(request.reserved, scale);
    const finished = request.finished_at
      ? m.account_receipt_finished({ time: time(request.finished_at) })
      : null;
    let line: string | null;
    switch (requestStanding(request.state)) {
      case "charged":
        line = m.account_receipt_charged({ amount: credits(request.charged ?? "0", scale) ?? "0" });
        break;
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
    return [line, finished].filter(Boolean).join(" · ");
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
    <ul>
      {#each requests as request (request.id)}
        <li>
          <button
            type="button"
            class="request"
            aria-expanded={open === request.id}
            aria-label={m.account_request_details()}
            onclick={() => void toggle(request.id)}
          >
            <span class="model">{request.model_id ?? "—"}</span>
            <span class="when">{time(request.started_at)}</span>
            <span class="cost" data-standing={requestStanding(request.state)}>{cost(request)}</span>
          </button>
          {#if open === request.id}<p class="receipt" role="status">
              {#if receipt === null}{m.account_usage_loading()}
              {:else if typeof receipt === "string"}{explainAccountFault(receipt, null)}
              {:else}{details(receipt)}{/if}
            </p>{/if}
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

  .request {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 8.5em;
    align-items: baseline;
    gap: 16px;
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

  .request:hover {
    background: var(--color-fill-hover);
  }

  .request[aria-expanded="true"] {
    background: var(--color-fill);
  }

  .cost {
    text-align: end;
  }

  .request:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: -2px;
  }

  .model {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
