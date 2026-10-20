<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { accountSession as session, creditsLeft, priceLabel } from "$domain/account";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import type { TopUpOffer } from "../../lib/plans";
  import AccountDialog from "./AccountDialog.svelte";

  let {
    open = $bindable(false),
    packs,
    per,
    custom,
  }: {
    open?: boolean;
    packs: readonly TopUpOffer[];
    /** Units in one credit, when the server states it. */
    per: string | null;
    /** An amount of the person's own, once the service can sell one; the packs stay beside it. */
    custom?: Snippet;
  } = $props();

  const busy = $derived(session.pending());
  let chosen = $state<string | null>(null);
  let buying = $state(false);
  const pack = $derived(packs.find((offer) => offer.id === chosen) ?? packs[0] ?? null);

  $effect(() => {
    if (!open) return;
    untrack(() => {
      buying = false;
      chosen = packs[0]?.id ?? null;
    });
  });

  const amount = (offer: TopUpOffer) => creditsLeft(offer.units, per);
  const price = (offer: TopUpOffer) => priceLabel(offer.price);

  const action = $derived.by(() => {
    if (!pack) return "";
    const credits = amount(pack);
    const cost = price(pack);
    if (credits && cost) return m.account_credits_buy_action({ amount: credits, price: cost });
    return cost ? m.account_credits_buy_price({ price: cost }) : m.account_topup_buy();
  });

  async function buy() {
    if (!pack || buying || busy) return;
    buying = true;
    await session.buy({ kind: "top_up", id: pack.id });
    buying = false;
    open = false;
  }
</script>

<AccountDialog
  bind:open
  title={m.account_credits_title()}
  description={m.account_topup_desc()}
  width={520}
>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      void buy();
    }}
  >
    <!-- Native radios: the arrow keys, the group's one tab stop and its announcement are the browser's. -->
    <fieldset class="packs">
      <legend class="hidden">{m.account_credits_packs()}</legend>
      {#each packs as offer (offer.id)}
        {@const credits = amount(offer)}
        <label class="pack">
          <input type="radio" name="pack" value={offer.id} bind:group={chosen} />
          <span class="credits">{credits ?? offer.name ?? m.account_topup_title()}</span>
          {#if credits}<span class="unit">{m.account_credits_unit()}</span>{/if}
          <span class="cost">{price(offer) ?? ""}</span>
          {#if offer.expires_days}<span class="lasts"
              >{m.account_credits_lasts({ days: offer.expires_days })}</span
            >{/if}
        </label>
      {/each}
    </fieldset>
    {#if custom}{@render custom()}{/if}
    <footer>
      <Button onclick={() => (open = false)} disabled={buying}>{m.action_cancel()}</Button>
      <Button type="submit" variant="primary" pending={buying} disabled={!pack || !!busy}
        >{action}</Button
      >
    </footer>
  </form>
</AccountDialog>

<style>
  .packs {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(132px, 1fr));
    gap: 10px;
    min-inline-size: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .hidden {
    position: absolute;
    overflow: hidden;
    inline-size: 1px;
    block-size: 1px;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .pack {
    position: relative;
    display: grid;
    align-content: start;
    gap: 2px;
    padding: 16px 16px 14px;
    border-radius: var(--radius-card);
    background: var(--color-surface);
    box-shadow: inset 0 0 0 1px var(--color-border);
    cursor: default;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      box-shadow var(--motion-fast) var(--ease-out);
  }

  .pack:hover {
    background: var(--row-hover);
  }

  .pack:has(input:checked) {
    box-shadow: inset 0 0 0 1.5px var(--color-accent);
    background: var(--color-accent-soft);
  }

  .pack:has(input:focus-visible) {
    outline: 2px solid var(--color-ring);
    outline-offset: 2px;
  }

  .pack input {
    position: absolute;
    opacity: 0;
    inset: 0;
    margin: 0;
    pointer-events: none;
  }

  .credits {
    color: var(--color-text);
    font-size: var(--text-headline);
    font-variant-numeric: tabular-nums;
    font-weight: 600;
    letter-spacing: -0.025em;
    line-height: 1.2;
  }

  .unit,
  .cost,
  .lasts {
    color: var(--color-muted);
    font-size: var(--text-label);
    font-variant-numeric: tabular-nums;
  }

  .cost {
    margin-block-start: 8px;
    color: var(--color-label-secondary);
  }

  .lasts {
    color: var(--color-faint);
    font-size: var(--text-caption);
  }

  @media (prefers-reduced-motion: reduce) {
    .pack {
      transition: none;
    }
  }

  @media (forced-colors: active) {
    .pack:has(input:checked) {
      outline: 2px solid Highlight;
    }
  }
</style>
