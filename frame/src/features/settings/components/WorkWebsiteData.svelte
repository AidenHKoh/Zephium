<script lang="ts">
  import { Dialog } from "bits-ui";
  import { commands } from "$shared/ipc/bindings";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import SettingsRow from "$shared/ui/SettingsRow";

  let { profile }: { profile: string } = $props();
  let open = $state(false);
  let pending = $state(false);
  let failed = $state(false);
  let cleared = $state(false);
  let opener: HTMLElement | null = null;

  async function clear() {
    if (pending) return;
    pending = true;
    failed = false;
    try {
      if (!(await commands.workClearWebsiteData(profile))) {
        failed = true;
        return;
      }
      cleared = true;
      open = false;
    } catch {
      failed = true;
    } finally {
      pending = false;
    }
  }
</script>

<div data-setting="work.website-data">
  <SettingsRow
    title={m.settings_work_website_data()}
    description={m.settings_work_website_data_desc()}
  >
    <Button
      onclick={() => {
        opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        cleared = false;
        failed = false;
        open = true;
      }}>{m.settings_work_website_data_open()}</Button
    >
  </SettingsRow>
  {#if cleared}<p class="result" role="status">{m.settings_work_website_data_done()}</p>{/if}
</div>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="settings-dialog-overlay" />
    <Dialog.Content
      class="settings-dialog"
      onCloseAutoFocus={(event) => {
        if (opener?.isConnected) {
          event.preventDefault();
          opener.focus();
        }
      }}
    >
      <Dialog.Title class="settings-dialog-title"
        >{m.settings_work_website_data_title()}</Dialog.Title
      >
      <Dialog.Description class="settings-dialog-description"
        >{m.settings_work_website_data_confirm()}</Dialog.Description
      >
      <form
        onsubmit={(event) => {
          event.preventDefault();
          void clear();
        }}
      >
        {#if failed}<p class="result error" role="alert">
            {m.settings_work_website_data_failed()}
          </p>{/if}
        <footer>
          <Button onclick={() => (open = false)}>{m.action_cancel()}</Button>
          <Button type="submit" variant="primary" {pending}
            >{m.settings_work_website_data_apply()}</Button
          >
        </footer>
      </form>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  .result {
    margin: 12px 18px;
    color: var(--color-muted);
    font-size: var(--text-body);
  }

  .error {
    color: var(--color-danger);
  }
</style>
