<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Icon from "$shared/ui/Icon";
  import { HandPointingRight01Icon, Layers01Icon } from "@hugeicons/core-free-icons";
  import "./assist.css";

  let {
    kind,
    onact,
  }: {
    /** `paused`: the person took over. `work`: the request is bigger than a panel. */
    kind: "paused" | "work";
    onact: () => void;
  } = $props();
</script>

<section
  class="assist-card assist-rise"
  aria-label={kind === "paused" ? m.assist_paused_title() : m.assist_work_title()}
>
  <div class="head">
    <span class="plate" aria-hidden="true"
      ><Icon icon={kind === "paused" ? HandPointingRight01Icon : Layers01Icon} size={15} /></span
    >
    <div class="words">
      <h4>{kind === "paused" ? m.assist_paused_title() : m.assist_work_title()}</h4>
      <p>{kind === "paused" ? m.assist_paused_help() : m.assist_work_help()}</p>
    </div>
  </div>
  <div class="assist-actions">
    <Button variant={kind === "work" ? "primary" : "secondary"} size="compact" onclick={onact}
      >{kind === "paused" ? m.assist_continue() : m.assist_work_run()}</Button
    >
  </div>
</section>

<style>
  .head {
    display: flex;
    gap: 10px;
  }

  .plate {
    display: grid;
    flex: none;
    place-items: center;
    inline-size: 28px;
    block-size: 28px;
    border-radius: var(--radius-inset);
    background: var(--color-field);
    color: var(--color-muted);
  }

  .words {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-inline-size: 0;
  }
</style>
