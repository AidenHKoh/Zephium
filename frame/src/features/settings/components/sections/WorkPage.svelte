<script lang="ts">
  import * as m from "$shared/i18n/messages";
  import { tabs } from "$domain/tabs";
  import { preferences } from "$domain/preferences";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import Switch from "$shared/ui/Switch";
  import WorkSwitch from "../WorkSwitch.svelte";
  import WorkWebsiteData from "../WorkWebsiteData.svelte";
  import WorkScreenshotSites from "../WorkScreenshotSites.svelte";
  let profile = $derived(tabs.profile());
  let regular = $derived(!!profile && profile.kind !== "incognito");
</script>

<WorkSwitch />
{#if preferences.aiEnabled()}
  <SettingsGroup title={m.settings_work_privacy()}>
    <SettingsRow
      settingId="work.screenshots"
      title={m.settings_work_screenshots()}
      description={m.settings_work_screenshots_desc()}
      ><Switch
        label={m.settings_work_screenshots()}
        labelHidden
        checked={preferences.value("work.screenshots") !== "false"}
        disabled={preferences.saving()}
        onchange={(on) => void preferences.set("work.screenshots", String(on))}
      /></SettingsRow
    >
    {#if profile && regular}
      <WorkScreenshotSites profile={profile.id} />
      <WorkWebsiteData profile={profile.id} />
    {/if}
  </SettingsGroup>
  {#if !regular}<p class="work-note">{m.work_regular_profile()}</p>{/if}
{/if}

<style>
  .work-note {
    margin: 0 16px;
    font-size: var(--text-label);
    line-height: 1.5;
    color: var(--color-muted);
  }
</style>
