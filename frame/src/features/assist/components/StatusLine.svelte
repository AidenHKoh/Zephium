<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { Orb, Shimmer } from "$shared/ui/presence";
  import type { PageAssistPhaseV1 } from "$shared/ipc/bindings";

  let { phase }: { phase: PageAssistPhaseV1 } = $props();
  const words = $derived(
    phase.phase === "reading"
      ? m.assist_status_reading()
      : phase.phase === "read"
        ? m.assist_status_read({ count: phase.passages })
        : phase.phase === "acting"
          ? m.assist_status_acting()
          : phase.phase === "styling"
            ? m.assist_status_styling()
            : m.assist_status_answering(),
  );
</script>

<p class="status assist-rise" role="status">
  <Orb size={14} /><span class="words"><Shimmer text={words} /></span>
</p>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-body);
    line-height: 20px;
  }

  .words {
    --shimmer-rest: var(--color-faint);
    --shimmer-lit: var(--color-text);

    min-inline-size: 0;
  }
</style>
