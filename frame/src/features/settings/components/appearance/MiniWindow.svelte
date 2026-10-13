<script lang="ts">
  import { palette, type Mode, type ThemeSpec } from "$domain/appearance";

  let {
    spec,
    contrast,
    mode,
    material = "none",
    tint = 0,
    inheritAccent = false,
    detail = true,
  }: {
    spec: ThemeSpec;
    contrast: number;
    mode: Mode;
    /** The window's material, drawn as frost over the theme's wallpaper. */
    material?: string;
    /** Window tint, 0 to 100. */
    tint?: number;
    /** A quick tint is set on the window: take its accent rather than the theme's. */
    inheritAccent?: boolean;
    /** The page's controls and the open menu; a thumbnail leaves them out. */
    detail?: boolean;
  } = $props();

  // The seeds and nothing else: the ladder itself comes from palette.css,
  // resolved inside this subtree, so the picture is the real tokens.
  let seeds = $derived.by(() => {
    const vars = Object.entries(palette.seedVars(spec, contrast)).map(
      ([name, value]) => `${name}:${value}`,
    );
    if (inheritAccent)
      for (const name of [
        "--color-accent",
        "--color-accent-hover",
        "--color-on-accent",
        "--color-track-on",
      ])
        vars.push(`${name}:inherit`);
    return vars.join(";");
  });
  let glass = $derived(material !== "none");
</script>

<div
  class="stage"
  data-theme-scope
  data-theme={mode}
  data-glass={glass}
  data-detail={detail}
  style={seeds}
  style:--tint={`${tint}%`}
  aria-hidden="true"
>
  <div class="window">
    <div class="sidebar">
      <div class="lights"><i></i><i></i><i></i></div>
      <div class="address"><span class="bar"></span></div>
      <div class="tiles"><i></i><i></i><i></i></div>
      <div class="tab">
        <b style:--site="var(--color-soft-sky)"></b><span class="bar" style:--w="64%"></span>
      </div>
      <div class="tab current">
        <b style:--site="var(--color-soft-peach)"></b><span class="bar" style:--w="72%"></span>
      </div>
      <div class="tab">
        <b style:--site="var(--color-soft-mint)"></b><span class="bar" style:--w="52%"></span>
      </div>
      <div class="tab">
        <b style:--site="var(--color-soft-lilac)"></b><span class="bar" style:--w="60%"></span>
      </div>
    </div>
    <div class="page">
      <div class="card">
        <span class="title"></span>
        <span class="line" style:--w="92%"></span>
        <span class="line" style:--w="78%"></span>
        <span class="line faint" style:--w="54%"></span>
        {#if detail}
          <div class="controls">
            <span class="primary"><span></span></span>
            <span class="secondary"><span></span></span>
            <span class="switch"><i></i></span>
          </div>
        {/if}
      </div>
      {#if detail}
        <div class="menu">
          <span class="item"><span class="entry" style:--w="58%"></span></span>
          <span class="item current"><span class="entry" style:--w="70%"></span></span>
          <span class="separator"></span>
          <span class="item"><span class="entry" style:--w="44%"></span></span>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  /* Everything is in units of --u, so the same window draws as the large
     preview and as a mode thumbnail. */
  .stage {
    --u: 1px;

    position: relative;
    overflow: hidden;
    height: 100%;
    border-radius: inherit;
    background:
      radial-gradient(
        60% 90% at 82% 8%,
        color-mix(in oklab, var(--seed-accent) 54%, var(--seed-bg)) 0%,
        transparent 70%
      ),
      radial-gradient(
        55% 80% at 8% 96%,
        color-mix(in oklab, var(--seed-accent) 34%, var(--seed-fg) 18%) 0%,
        transparent 72%
      ),
      linear-gradient(
        165deg,
        color-mix(in oklab, var(--seed-fg) 10%, var(--seed-bg)),
        color-mix(in oklab, var(--seed-bg) 88%, black)
      );
    color: var(--color-text);
  }

  .stage[data-theme="light"] {
    background:
      radial-gradient(
        60% 90% at 82% 8%,
        color-mix(in oklab, var(--seed-accent) 30%, white) 0%,
        transparent 70%
      ),
      radial-gradient(
        55% 80% at 8% 96%,
        color-mix(in oklab, var(--seed-accent) 24%, var(--seed-bg)) 0%,
        transparent 72%
      ),
      linear-gradient(165deg, white, var(--seed-bg));
  }

  .stage[data-detail="false"] {
    --u: 0.42px;
  }

  .window {
    position: absolute;
    inset: calc(34 * var(--u)) calc(44 * var(--u)) calc(-28 * var(--u));
    display: flex;
    gap: calc(8 * var(--u));
    padding: calc(8 * var(--u));
    border-radius: calc(14 * var(--u));
    background: var(--color-chrome);
    box-shadow: var(--shadow-overlay);
  }

  .stage[data-detail="false"] .window {
    inset: calc(26 * var(--u)) calc(-30 * var(--u)) calc(-30 * var(--u)) calc(30 * var(--u));
  }

  /* Over glass the column is the frost and whatever tint was asked for,
     with the wallpaper coming through. */
  .stage[data-glass="true"] .window {
    background:
      linear-gradient(
        color-mix(in srgb, var(--color-chrome) var(--tint), transparent),
        color-mix(in srgb, var(--color-chrome) var(--tint), transparent)
      ),
      color-mix(in srgb, var(--seed-bg) 56%, transparent);
    backdrop-filter: blur(calc(24 * var(--u))) saturate(1.4);
  }

  .stage[data-theme="light"][data-glass="true"] .window {
    background:
      linear-gradient(
        color-mix(in srgb, var(--color-chrome) var(--tint), transparent),
        color-mix(in srgb, var(--color-chrome) var(--tint), transparent)
      ),
      color-mix(in srgb, var(--color-chrome) 74%, transparent);
  }

  .sidebar {
    display: flex;
    flex: 0 0 calc(196 * var(--u));
    flex-direction: column;
    gap: calc(4 * var(--u));
  }

  .lights {
    display: flex;
    gap: calc(7 * var(--u));
    height: calc(24 * var(--u));
    padding-inline-start: calc(6 * var(--u));
    align-items: center;
  }

  .lights i {
    width: calc(11 * var(--u));
    height: calc(11 * var(--u));
    border-radius: 50%;
    background: color-mix(in srgb, var(--color-text) 22%, transparent);
  }

  .bar {
    display: block;
    width: var(--w, 60%);
    height: calc(7 * var(--u));
    border-radius: calc(4 * var(--u));
  }

  .address {
    display: flex;
    align-items: center;
    height: calc(30 * var(--u));
    margin-block: calc(4 * var(--u)) calc(6 * var(--u));
    padding-inline: calc(12 * var(--u));
    border-radius: calc(11 * var(--u));
    background: var(--color-address);
  }

  .address .bar {
    --w: 46%;

    background: var(--color-muted);
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: calc(6 * var(--u));
    margin-block-end: calc(8 * var(--u));
  }

  .tiles i {
    height: calc(36 * var(--u));
    border-radius: calc(11 * var(--u));
    background: var(--color-tile);
  }

  .tab {
    display: flex;
    align-items: center;
    gap: calc(10 * var(--u));
    height: calc(32 * var(--u));
    padding-inline: calc(10 * var(--u));
    border-radius: calc(10 * var(--u));
  }

  .tab b {
    flex: none;
    width: calc(14 * var(--u));
    height: calc(14 * var(--u));
    border-radius: calc(4 * var(--u));
    background: var(--site);
    opacity: var(--favicon-rest-opacity);
  }

  .tab .bar {
    background: var(--color-label-secondary);
    opacity: 0.55;
  }

  .tab.current {
    background: var(--row-active);
    box-shadow: var(--row-rim);
  }

  .tab.current .bar {
    background: var(--color-text);
    opacity: 0.9;
  }

  .page {
    position: relative;
    flex: 1;
    min-width: 0;
    padding: calc(30 * var(--u)) calc(34 * var(--u));
    border-radius: calc(10 * var(--u));
    background: var(--color-page);
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: calc(9 * var(--u));
    width: calc(300 * var(--u));
    max-width: 100%;
    box-sizing: border-box;
    padding: calc(18 * var(--u));
    border-radius: calc(16 * var(--u));
    background: var(--color-surface);
    box-shadow: var(--shadow-card);
  }

  .stage[data-detail="true"] .card {
    max-width: calc(100% - 150 * var(--u));
  }

  .title {
    width: 48%;
    height: calc(11 * var(--u));
    margin-block-end: calc(4 * var(--u));
    border-radius: calc(5 * var(--u));
    background: var(--color-text);
  }

  .line {
    width: var(--w);
    height: calc(7 * var(--u));
    border-radius: calc(4 * var(--u));
    background: var(--color-muted);
    opacity: 0.7;
  }

  .line.faint {
    background: var(--color-faint);
  }

  .controls {
    display: flex;
    align-items: center;
    gap: calc(8 * var(--u));
    margin-block-start: calc(10 * var(--u));
  }

  .primary,
  .secondary {
    display: grid;
    place-items: center;
    width: calc(76 * var(--u));
    height: calc(28 * var(--u));
    border-radius: calc(10 * var(--u));
  }

  .primary {
    background: var(--color-accent);
  }

  .secondary {
    width: calc(64 * var(--u));
    background: var(--color-control);
    box-shadow: var(--shadow-control);
  }

  .primary span,
  .secondary span {
    width: 46%;
    height: calc(6 * var(--u));
    border-radius: calc(3 * var(--u));
    background: var(--color-on-accent);
  }

  .secondary span {
    background: var(--color-on-control);
    opacity: 0.7;
  }

  .switch {
    position: relative;
    width: calc(34 * var(--u));
    height: calc(20 * var(--u));
    margin-inline-start: auto;
    border-radius: calc(10 * var(--u));
    background: var(--color-track-on);
  }

  .switch i {
    position: absolute;
    inset-block-start: calc(2 * var(--u));
    inset-inline-end: calc(2 * var(--u));
    width: calc(16 * var(--u));
    height: calc(16 * var(--u));
    border-radius: 50%;
    background: var(--color-switch-thumb);
    box-shadow: var(--shadow-thumb);
  }

  .menu {
    position: absolute;
    inset-block-start: calc(58 * var(--u));
    inset-inline-end: calc(22 * var(--u));
    display: flex;
    flex-direction: column;
    gap: calc(2 * var(--u));
    width: calc(150 * var(--u));
    padding: calc(5 * var(--u));
    border-radius: calc(14 * var(--u));
    background: var(--color-float);
    box-shadow: var(--shadow-menu);
  }

  .item {
    display: flex;
    align-items: center;
    height: calc(24 * var(--u));
    padding-inline: calc(9 * var(--u));
    border-radius: calc(8 * var(--u));
  }

  .entry {
    display: block;
    width: var(--w);
    border-radius: calc(4 * var(--u));
    height: calc(6 * var(--u));
    background: var(--color-text);
    opacity: 0.6;
  }

  .item.current {
    background: var(--row-active);
  }

  .item.current .entry {
    opacity: 0.9;
  }

  .separator {
    height: 1px;
    margin: calc(3 * var(--u)) calc(9 * var(--u));
    background: var(--color-menu-separator);
  }
</style>
