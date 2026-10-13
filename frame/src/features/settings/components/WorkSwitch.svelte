<script lang="ts">
  import { Briefcase02Icon } from "@hugeicons/core-free-icons";
  import * as m from "$shared/i18n/messages";
  import { preferences } from "$domain/preferences";
  import { commands } from "$shared/ipc/bindings";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import Switch from "$shared/ui/Switch";

  const on = $derived(preferences.aiEnabled());
  // Off is asked first and held off while it saves; a refusal brings it back.
  let asking = $state(false);
  let leaving = $state(false);
  // Read when the question opens, so it says whether running work stops.
  let running = $state(false);
  let dialog = $state<HTMLDialogElement>();
  const uid = $props.id();
  // The platform's own modal: the page behind goes inert, Escape cancels, and
  // focus returns to the switch when it closes.
  $effect(() => {
    if (!dialog) return;
    if (asking && !dialog.open) dialog.showModal();
    else if (!asking && dialog.open) dialog.close();
  });

  const leaves = [
    m.ai_switch_leaves_work,
    m.ai_switch_leaves_models,
    m.ai_switch_leaves_dictation,
    m.ai_switch_leaves_settings,
  ];

  async function turnOn() {
    await preferences.set("ai.enabled", "true");
    if (preferences.value("work.enabled") === "false")
      await preferences.set("work.enabled", "true");
  }

  async function turnOff() {
    asking = false;
    leaving = true;
    await preferences.set("ai.enabled", "false");
    leaving = false;
  }
</script>

<div class="work-switch" data-setting="ai.enabled" data-on={on}>
  <span class="tile" aria-hidden="true"><Icon icon={Briefcase02Icon} size={20} /></span>
  <div class="copy">
    <h3>{m.ai_switch_title()}</h3>
    <p>{on ? m.ai_switch_on() : m.ai_switch_off()}</p>
  </div>
  <Switch
    label={m.ai_switch_title()}
    labelHidden
    checked={on && !asking && !leaving}
    disabled={preferences.saving()}
    onchange={(next) => {
      if (next) void turnOn();
      else {
        running = false;
        asking = true;
        void Promise.resolve()
          .then(() => commands.workRunning())
          .then((now) => (running = now))
          .catch(() => {});
      }
    }}
  />
</div>

<dialog
  bind:this={dialog}
  class="confirm settings-modal"
  aria-labelledby={`${uid}-title`}
  aria-describedby={`${uid}-body`}
  oncancel={() => (asking = false)}
  onclose={() => (asking = false)}
  onclick={(event) => {
    if (event.target === dialog) asking = false;
  }}
>
  <h2 id={`${uid}-title`}>{m.ai_switch_confirm_title()}</h2>
  <p id={`${uid}-body`}>{m.ai_switch_confirm_body()}</p>
  {#if running}<p class="stops" role="status">{m.ai_switch_stops_running()}</p>{/if}
  <ul class="leaves">
    {#each leaves as leave, index (index)}<li>{leave()}</li>{/each}
  </ul>
  <footer>
    <Button onclick={() => (asking = false)}>{m.action_cancel()}</Button>
    <Button variant="primary" onclick={() => void turnOff()}>{m.ai_switch_confirm_action()}</Button>
  </footer>
</dialog>

<style>
  .work-switch {
    display: flex;
    align-items: center;
    gap: 14px;
    box-sizing: border-box;
    margin-block-end: 34px;
    padding: 16px 18px 16px 16px;
    border-radius: var(--radius-card);
    background: var(--color-surface);
    box-shadow: var(--shadow-card);
  }

  .tile {
    display: grid;
    flex: none;
    inline-size: 40px;
    block-size: 40px;
    border-radius: var(--radius-control);
    background: var(--color-fill);
    color: var(--color-text);
    place-items: center;
    transition: color var(--motion-base) var(--ease-out);
  }

  .work-switch[data-on="false"] .tile {
    color: var(--color-muted);
  }

  .copy {
    flex: 1;
    min-width: 0;
  }

  h3 {
    margin: 0;
    color: var(--color-text);
    font-size: 15px;
    font-weight: 600;
    line-height: 20px;
    letter-spacing: -0.012em;
  }

  p {
    margin: 3px 0 0;
    color: var(--color-muted);
    font-size: var(--text-label);
    line-height: 1.5;
  }

  .confirm {
    box-sizing: border-box;
    inline-size: min(480px, calc(100vw - 48px));
    max-block-size: calc(100vh - 48px);
    margin: auto;
    padding: 26px;
    overflow-y: auto;
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-card);
    color: var(--color-text);
    background: var(--color-raised);
    box-shadow: var(--shadow-overlay);
  }

  .confirm[open] {
    animation: interface-fade var(--motion-base) var(--ease-out);
  }

  .confirm h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 550;
    line-height: 1.3;
    letter-spacing: -0.02em;
  }

  .confirm > p {
    margin: 10px 0 22px;
    color: var(--color-muted);
    font-size: 12px;
    line-height: 1.6;
  }

  .confirm footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-block-start: 28px;
  }

  .stops {
    margin: -8px 0 18px;
    color: var(--color-text);
    font-size: var(--text-label);
    font-weight: 500;
    line-height: 1.5;
  }

  .leaves {
    display: grid;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .leaves li {
    position: relative;
    padding-inline-start: 16px;
    color: var(--color-text);
    font-size: var(--text-label);
    line-height: 1.5;
  }

  .leaves li::before {
    content: "";
    position: absolute;
    inset-block-start: 0.62em;
    inset-inline-start: 2px;
    inline-size: 5px;
    block-size: 5px;
    border-radius: 50%;
    background: var(--color-faint);
  }

  @media (prefers-reduced-motion: reduce) {
    .confirm[open] {
      animation: none;
    }
  }

  @media (forced-colors: active) {
    .confirm {
      border-color: CanvasText;
    }

    .work-switch {
      border: 1px solid ButtonText;
      box-shadow: none;
    }
  }
</style>
