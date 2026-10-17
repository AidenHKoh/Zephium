<script lang="ts">
  import { Select } from "bits-ui";
  import Mark from "../Menu/Mark.svelte";
  import "../Menu/popover.css";
  import "../Shine/shine.css";
  import "../Button/control.css";
  import { menuMaterialFor } from "../Menu/material";
  let {
    label,
    labelHidden = false,
    options,
    value = $bindable(""),
    disabled = false,
    onchange,
  }: {
    label: string;
    labelHidden?: boolean;
    options: ReadonlyArray<{ value: string; label: string; disabled?: boolean }>;
    value?: string;
    disabled?: boolean;
    onchange?: (value: string) => void;
  } = $props();
  const uid = $props.id();
  let triggerElement = $state<HTMLButtonElement | null>(null);
  let material = $state<"opaque" | undefined>();
  let current = $derived(options.find((option) => option.value === value)?.label ?? "");
</script>

<div class="field">
  <label for={uid} class:sr-only={labelHidden}>{label}</label>
  <Select.Root
    onOpenChange={(open) => {
      if (open) material = menuMaterialFor(triggerElement);
    }}
    type="single"
    bind:value
    {disabled}
    items={[...options]}
    onValueChange={(next) => onchange?.(next)}
  >
    <Select.Trigger
      bind:ref={triggerElement}
      id={uid}
      class="ui-select ui-control ui-shine ui-shine-press"
      aria-label={label}
    >
      <span class="value">{current}</span>
      <svg
        class="chevron"
        aria-hidden="true"
        viewBox="0 0 12 16"
        width="11"
        height="14"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M3 6.5L6 3.5l3 3" />
        <path d="M3 9.5l3 3 3-3" />
      </svg>
    </Select.Trigger>
    <Select.Portal>
      <Select.Content
        data-menu-material={material}
        class="ui-menu ui-select-menu"
        align="start"
        sideOffset={6}
        collisionPadding={12}
      >
        <Select.Viewport class="ui-menu-viewport">
          {#each options as option (option.value)}
            <Select.Item
              class="ui-menu-item"
              value={option.value}
              label={option.label}
              disabled={option.disabled}
            >
              {#snippet children({ selected })}
                {option.label}
                <Mark on={selected} />
              {/snippet}
            </Select.Item>
          {/each}
        </Select.Viewport>
      </Select.Content>
    </Select.Portal>
  </Select.Root>
</div>

<style>
  .field {
    display: grid;
    gap: 6px;
    min-width: 0;
  }

  .field > label {
    font-size: var(--text-label);
    font-weight: 500;
    color: var(--color-muted);
  }

  /* The one control size, on the shine plate. The text inset matches the
     menu's own, so the label does not move when it opens. */
  .field :global(.ui-select) {
    justify-content: space-between;
    gap: 8px;
    min-width: 120px;
    padding-inline: 10px 6px;
    font-weight: 450;
  }

  .value {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-align: start;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Two chevrons, because this one picks from a set rather than opening a
     list of commands — the platform's own distinction, and the fastest way to
     tell the two apart without reading either. */
  .chevron {
    flex: none;
    color: var(--color-muted);
  }

  /* At least the trigger's width, so the label it is covering never has to
     move sideways to fit. */
  :global(.ui-select-menu) {
    min-width: var(--bits-floating-anchor-width, 0);
  }
</style>
