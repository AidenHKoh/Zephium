<script lang="ts">
  import PreferenceSelect from "../PreferenceSelect.svelte";
  import * as m from "$shared/i18n/messages";
  import { preferences } from "$domain/preferences";
  import Select from "$shared/ui/Select";
  import Switch from "$shared/ui/Switch";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import Icon from "$shared/ui/Icon";
  import { Tick02Icon } from "@hugeicons/core-free-icons";
  import { onMount } from "svelte";
  import { IS_WINDOWS } from "$shared/platform";
  let material = $state("");
  onMount(() => {
    const update = () => {
      material = document.documentElement.dataset.material ?? "none";
    };
    update();
    const observer = new MutationObserver(update);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-material"],
    });
    return () => observer.disconnect();
  });
  const themes = [
    { value: "light", label: m.theme_light },
    { value: "dark", label: m.theme_dark },
    { value: "system", label: m.theme_system },
  ];
  const tints = [
    { id: "graphite", label: m.settings_graphite },
    { id: "sky", label: m.settings_sky },
    { id: "sage", label: m.settings_sage },
    { id: "rose", label: m.settings_rose },
    { id: "amber", label: m.settings_amber },
    { id: "teal", label: m.settings_teal },
    { id: "lavender", label: m.settings_lavender },
    { id: "orchid", label: m.settings_orchid },
  ];
  // What the person may choose here. Windows has one material worth having;
  // a Mac offers the glass, the older vibrancy, or neither.
  const materialOptions = IS_WINDOWS
    ? [
        { value: "auto", label: m.settings_material_acrylic() },
        { value: "none", label: m.settings_material_solid() },
      ]
    : [
        { value: "auto", label: m.settings_material_auto() },
        { value: "liquid_glass", label: m.settings_material_glass() },
        { value: "vibrancy", label: m.settings_material_vibrancy() },
        { value: "none", label: m.settings_material_solid() },
      ];
  // A choice the system cannot install right now, such as glass on an older
  // macOS or any material under Reduce Transparency, says what is showing.
  let fallback = $derived.by(() => {
    const chosen = preferences.value("ui.material");
    return chosen !== "auto" && material !== "" && chosen !== material;
  });
  const materials: Record<string, () => string> = {
    liquid_glass: m.settings_material_glass,
    vibrancy: m.settings_material_vibrancy,
    acrylic: m.settings_material_acrylic,
    mica: m.settings_material_mica,
    none: m.settings_material_solid,
  };
</script>

<SettingsGroup title={m.settings_theme()}>
  <!-- The pictures are the choice, as the system's own Appearance pane draws
       it: one control, named under each picture. -->
  <div
    class="appearance-previews"
    role="radiogroup"
    aria-label={m.settings_color_scheme()}
    data-setting="appearance.theme"
  >
    {#each themes as theme (theme.value)}
      {@const selected = preferences.value("appearance") === theme.value}
      <button
        type="button"
        role="radio"
        class="appearance-choice"
        aria-checked={selected}
        disabled={preferences.saving()}
        onclick={() => void preferences.set("appearance", theme.value)}
      >
        <span class="appearance-preview" data-selected={selected} aria-hidden="true">
          {#each theme.value === "system" ? ["light", "dark"] : [theme.value] as tone (tone)}
            <span class="mini-window" data-tone={tone}>
              <span class="mini-sidebar"><span></span><i></i><i></i><i></i></span>
              <span class="mini-page"><span></span><i></i><i></i></span>
            </span>
          {/each}
        </span>
        <span class="appearance-label">{theme.label()}</span>
      </button>
    {/each}
  </div>
  <SettingsRow
    settingId="appearance.accent"
    title={m.settings_accent()}
    description={m.settings_accent_desc()}
  >
    <div class="accent-choices" role="group" aria-label={m.settings_accent()}>
      {#each tints as tint (tint.id)}<button
          class="accent-swatch"
          style:--swatch={`var(--color-tint-${tint.id})`}
          aria-label={tint.label()}
          title={tint.label()}
          aria-pressed={preferences.value("ui.accent") === tint.id}
          disabled={preferences.saving()}
          onclick={() => void preferences.set("ui.accent", tint.id)}
          >{#if preferences.value("ui.accent") === tint.id}<Icon
              icon={Tick02Icon}
              size={14}
            />{/if}</button
        >{/each}
    </div>
  </SettingsRow>
</SettingsGroup>
<SettingsGroup title={m.settings_layout()}>
  <SettingsRow
    settingId="appearance.sidebar"
    title={m.settings_sidebar()}
    description={m.settings_sidebar_desc()}
  >
    <Select
      label={m.settings_sidebar()}
      labelHidden
      value={preferences.value("sidebar.mode")}
      disabled={preferences.saving()}
      options={[
        { value: "default", label: m.settings_expanded() },
        { value: "compact", label: m.settings_compact() },
      ]}
      onchange={(v) => void preferences.set("sidebar.mode", v)}
    />
  </SettingsRow>
  <SettingsRow
    settingId="appearance.material"
    title={m.settings_material()}
    description={fallback
      ? m.settings_material_fallback({ material: materials[material]?.() ?? "" })
      : m.settings_material_desc()}
    ><Select
      label={m.settings_material()}
      labelHidden
      value={preferences.value("ui.material")}
      disabled={preferences.saving()}
      options={materialOptions}
      onchange={(v) => void preferences.set("ui.material", v)}
    /></SettingsRow
  >
  <PreferenceSelect id="appearance.density" preference="ui.density" />
</SettingsGroup>
<SettingsGroup title={m.settings_motion()}>
  <SettingsRow
    settingId="appearance.motion"
    title={m.settings_reduce_motion()}
    description={m.settings_reduce_motion_desc()}
    ><Switch
      label={m.settings_reduce_motion()}
      labelHidden
      checked={preferences.value("ui.reduce-motion") === "true"}
      disabled={preferences.saving()}
      onchange={(v) => void preferences.set("ui.reduce-motion", String(v))}
    /></SettingsRow
  >
</SettingsGroup>
