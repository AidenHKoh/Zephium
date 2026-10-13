<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  let maximized = $state(false);
  let focused = $state(true);

  onMount(() => {
    const current = getCurrentWindow();
    let disposed = false;
    const stops: (() => void)[] = [];
    const sync = () =>
      void current.isMaximized().then((value) => {
        if (!disposed) maximized = value;
      });
    sync();
    void current.onResized(sync).then((stop) => (disposed ? stop() : stops.push(stop)));
    void current
      .onFocusChanged(({ payload }) => (focused = payload))
      .then((stop) => (disposed ? stop() : stops.push(stop)));
    return () => {
      disposed = true;
      for (const stop of stops) stop();
    };
  });

  const window = () => getCurrentWindow();
</script>

<!--
  The system's own caption buttons, drawn by chrome: the platform's glyphs,
  sizes and red close, flush with the window's corner so a flung pointer
  still lands on Close. Only the ground is ours, which is the band itself.
-->
<div class="caption" data-inactive={!focused || undefined}>
  <button
    type="button"
    aria-label={m.window_minimize()}
    title={m.window_minimize()}
    onclick={() => void window().minimize()}>&#xE921;</button
  >
  <button
    type="button"
    aria-label={maximized ? m.window_restore() : m.window_maximize()}
    title={maximized ? m.window_restore() : m.window_maximize()}
    onclick={() => void window().toggleMaximize()}>{maximized ? "" : ""}</button
  >
  <button
    type="button"
    class="close"
    aria-label={m.window_close()}
    title={m.window_close()}
    onclick={() => void window().close()}>&#xE8BB;</button
  >
</div>

<style>
  .caption {
    display: flex;
    height: 100%;
  }

  button {
    display: grid;
    place-items: center;
    width: 46px;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--color-text);
    font-family: "Segoe Fluent Icons", "Segoe MDL2 Assets", sans-serif;
    font-size: 10px;
    cursor: default;
    outline: none;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out);
  }

  button:hover {
    background: var(--color-fill-hover);
    color: var(--color-text);
  }

  button:active {
    background: var(--color-fill-pressed);
    color: var(--color-muted);
  }

  /* The platform's close red, and the only hue in the band. */
  .close:hover {
    background: var(--color-close);
    color: var(--color-on-close);
  }

  .close:active {
    background: var(--color-close-pressed);
    color: var(--color-on-close);
  }

  button:focus-visible {
    outline: 2px solid var(--color-ring);
    outline-offset: -2px;
  }

  /* An inactive window's glyphs step back, as the system's own do. */
  .caption[data-inactive] button:not(:hover, :active) {
    color: var(--color-faint);
  }

  @media (forced-colors: active) {
    button:hover,
    .close:hover {
      background: Highlight;
      color: HighlightText;
    }
  }
</style>
