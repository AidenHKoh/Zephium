<script lang="ts">
  import "$styles/global.css";
  import type { AssistSession } from "../lib/assist-session.svelte";
  import AssistComposer from "../components/AssistComposer.svelte";
  import AssistThread from "../components/AssistThread.svelte";

  let { session }: { session: AssistSession } = $props();
  let draft = $state("");
</script>

<aside class="browser-sidebar stage">
  <header><h2>Ask</h2></header>
  <AssistThread {session} />
  <AssistComposer {session} {draft} ondraft={(text) => (draft = text)} />
</aside>

<style>
  .stage {
    display: flex;
    flex-direction: column;
    inline-size: 336px;
    block-size: 100%;
    box-sizing: border-box;
    color: var(--color-text);
  }

  header {
    display: flex;
    align-items: center;
    min-block-size: 48px;
    padding: 6px 16px;
    flex: none;
  }

  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 550;
    letter-spacing: -0.015em;
  }
</style>
