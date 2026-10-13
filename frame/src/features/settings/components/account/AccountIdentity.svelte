<script lang="ts">
  import { tick } from "svelte";
  import { PencilEdit02Icon } from "@hugeicons/core-free-icons";
  import { accountSession as session, initials, planName } from "$domain/account";
  import { identityAvatar, profiles } from "$domain/profiles";
  import * as m from "$shared/i18n/messages";
  import Button from "$shared/ui/Button";
  import Field from "$shared/ui/Field";
  import Icon from "$shared/ui/Icon";
  import ProfileAvatar from "$shared/ui/ProfileAvatar";
  import { explainAccountFault } from "../../lib/account";

  const account = $derived(session.account());
  const busy = $derived(session.pending());
  const fault = $derived(session.fault());
  const plan = $derived(planName(account?.plan));
  const avatar = $derived(
    identityAvatar(profiles.current(), initials(account?.email, account?.name)),
  );
  let editing = $state(false);
  let name = $state("");
  let form = $state<HTMLFormElement | null>(null);
  let opener = $state<HTMLButtonElement | null>(null);
  const faultText = $derived(
    fault?.action === "name" ? explainAccountFault(fault.fault, fault.retryAfter) : null,
  );

  async function start() {
    session.clearFault();
    name = account?.name ?? "";
    editing = true;
    await tick();
    form?.querySelector("input")?.select();
  }

  async function stop() {
    editing = false;
    await tick();
    opener?.focus();
  }

  async function save() {
    const trimmed = name.trim();
    if (await session.setName(trimmed ? trimmed : null)) await stop();
  }
</script>

<header class="account-identity">
  <ProfileAvatar view={avatar} size={64} />
  <div class="who">
    {#if editing}<form
        class="editor"
        bind:this={form}
        onsubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <Field
          label={m.account_name_title()}
          labelHidden
          autocomplete="name"
          placeholder={m.account_name_add()}
          maxlength={100}
          bind:value={name}
          onkeydown={(event) => {
            if (event.key === "Escape") void stop();
          }}
        />
        <Button size="compact" variant="ghost" onclick={() => void stop()}
          >{m.account_name_cancel()}</Button
        >
        <Button size="compact" variant="primary" type="submit" pending={busy === "name"}
          >{m.account_name_save()}</Button
        >
      </form>
    {:else}<div class="name">
        <h2>{account?.name ?? account?.email}</h2>
        <button
          type="button"
          class="edit"
          bind:this={opener}
          disabled={!!busy}
          aria-label={m.account_name_edit_label()}
          title={m.account_name_edit_label()}
          onclick={() => void start()}><Icon icon={PencilEdit02Icon} size={15} /></button
        >
      </div>{/if}
    <p>
      {#if account?.name}<span>{account.email}</span>{/if}
      {#if plan}<span class="plan">{m.account_identity_plan({ plan })}</span>{/if}
    </p>
    {#if faultText}<p class="error" role="alert">{faultText}</p>{/if}
  </div>
</header>

<style>
  .account-identity {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 4px 0 34px;
  }

  .who {
    display: grid;
    flex: 1;
    gap: 6px;
    min-inline-size: 0;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    min-inline-size: 0;
  }

  h2 {
    overflow: hidden;
    margin: 0;
    color: var(--color-text);
    font-size: 22px;
    font-weight: 600;
    line-height: 1.25;
    letter-spacing: -0.025em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .edit {
    display: grid;
    flex: none;
    inline-size: 26px;
    block-size: 26px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--color-faint);
    place-items: center;
    transition:
      background-color var(--motion-fast) var(--ease-out),
      color var(--motion-fast) var(--ease-out);
  }

  .edit:hover:not(:disabled) {
    background: var(--row-hover);
    color: var(--color-text);
  }

  .editor {
    display: flex;
    align-items: center;
    gap: 8px;
    max-inline-size: 440px;
  }

  .editor :global(.ui-field) {
    flex: 1;
  }

  p {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: var(--color-muted);
    font-size: var(--text-body);
  }

  .plan {
    padding: 2px 8px;
    border-radius: var(--radius-capsule);
    background: var(--color-fill);
    color: var(--color-label-secondary);
    font-size: var(--text-caption);
    font-weight: 600;
    letter-spacing: 0.01em;
  }

  .error {
    color: var(--color-danger);
    font-size: var(--text-label);
  }
</style>
