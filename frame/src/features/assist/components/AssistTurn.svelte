<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import type { AssistSession } from "../lib/assist-session.svelte";
  import type { TurnView } from "../lib/thread-model";
  import AnswerCard from "./AnswerCard.svelte";
  import CostLine from "./CostLine.svelte";
  import FaultNote from "./FaultNote.svelte";
  import StatusLine from "./StatusLine.svelte";
  import StepList from "./StepList.svelte";
  import StyleCard from "./StyleCard.svelte";
  import TurnCards from "./TurnCards.svelte";
  import "./assist.css";

  let { turn, session }: { turn: TurnView; session: AssistSession } = $props();

  const style = $derived(turn.style);
  // An applied style is on until the core's list says it was switched off.
  const enabled = $derived(session.styles.find((row) => row.id === style?.id)?.enabled ?? true);
</script>

<section class="turn" aria-label={turn.question}>
  {#if turn.question}<p class="ask assist-rise">{turn.question}</p>{/if}
  {#if turn.state === "working" && turn.phase}<StatusLine phase={turn.phase} />{/if}
  {#if turn.parts.length}<AnswerCard
      parts={turn.parts}
      inert={turn.inert}
      oncite={(n) => session.cite(turn.turn, n)}
    />{/if}
  {#if turn.steps.length}<StepList
      steps={turn.steps}
      site={session.site}
      ondecide={(approve) => session.decide(turn.turn, approve)}
    />{/if}
  {#if turn.state === "paused"}<TurnCards
      kind="paused"
      onact={() => session.resume(turn.turn)}
    />{/if}
  {#if turn.offer}<TurnCards kind="work" onact={() => session.handoff(turn.turn)} />{/if}
  {#if style}<StyleCard
      site={style.site}
      summary={style.summary}
      state={style.state}
      {enabled}
      ondecide={(approve) => session.decide(turn.turn, approve)}
      ontoggle={(next) => style.id && session.toggleStyle(style.id, next)}
      onremove={() => style.id && void session.removeStyle(style.id)}
    />{/if}
  {#if turn.state === "failed" && turn.fault}<FaultNote
      fault={turn.fault}
      onretry={() => void session.retry()}
    />{/if}
  {#if turn.state === "cancelled"}<p class="stopped assist-rise">{m.assist_stopped()}</p>{/if}
  {#if turn.state === "done" && turn.cost}<CostLine cost={turn.cost} />{/if}
</section>

<style>
  .turn {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  /* What was asked, set apart like a quotation from the page it is about. */
  .ask {
    align-self: flex-start;
    max-inline-size: 100%;
    margin: 0;
    padding-inline-start: 10px;
    border-inline-start: 2px solid var(--color-border-strong);
    color: var(--color-muted);
    font-size: 12.5px;
    line-height: 1.45;
    overflow-wrap: anywhere;
    text-wrap: pretty;
  }

  .stopped {
    margin: 0;
    color: var(--color-faint);
    font-size: 12.5px;
  }
</style>
