<script lang="ts">
  import { accountSession as session } from "$domain/account";
  import { commands } from "$shared/ipc/bindings";
  import * as m from "$shared/i18n/messages";
  import ZephiumTile from "$shared/ui/ZephiumTile";
  import Button from "$shared/ui/Button";
  import Field from "$shared/ui/Field";
  import {
    PRIVACY_URL,
    TERMS_URL,
    codeDigits,
    explainAccountFault,
    plausibleEmail,
  } from "../../lib/account";

  const account = $derived(session.account());
  const pendingEmail = $derived(account?.pending_email ?? null);
  const fault = $derived(session.fault());
  const busy = $derived(session.pending());

  let email = $state("");
  let code = $state("");
  let resent = $state(false);
  let emailForm = $state<HTMLFormElement | null>(null);
  let codeForm = $state<HTMLFormElement | null>(null);
  const focus = (form: HTMLFormElement | null) => form?.querySelector("input")?.focus();
  let now = $state(Date.now());
  const resendAt = $derived(Date.now() + (account?.resend_in ?? 0) * 1000);
  $effect(() => {
    if (resendAt <= now) return;
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });
  const resendIn = $derived(Math.max(0, Math.ceil((resendAt - now) / 1000)));

  $effect(() => {
    if (pendingEmail) focus(codeForm);
  });

  const faultText = $derived(
    fault && (fault.action === "send" || fault.action === "verify")
      ? explainAccountFault(fault.fault, fault.retryAfter)
      : null,
  );
  const emailValid = $derived(plausibleEmail(email));

  async function send() {
    if (!emailValid || busy) return;
    resent = false;
    await session.sendCode(email.trim());
  }

  async function resend() {
    if (!pendingEmail || busy || resendIn > 0) return;
    code = "";
    resent = await session.sendCode(pendingEmail);
    focus(codeForm);
  }

  async function verify() {
    const digits = codeDigits(code);
    if (digits.length !== 6 || busy) return;
    resent = false;
    const ok = await session.verify(digits);
    if (!ok) {
      code = "";
      focus(codeForm);
    }
  }

  function typed(value: string) {
    const digits = codeDigits(value);
    code = digits;
    if (digits.length === 6) void verify();
  }

  async function otherEmail() {
    code = "";
    resent = false;
    if (pendingEmail) email = pendingEmail;
    await session.cancelCode();
    focus(emailForm);
  }

  function open(url: string) {
    void commands.browserOpenUrl(url, true).catch(() => {});
  }
</script>

<div class="sign-in">
  <div class="mark"><ZephiumTile size={52} /></div>
  {#if !pendingEmail}
    <h2>{m.account_intro_title()}</h2>
    <p class="body">{m.account_intro_body()}</p>
    <form
      class="row"
      bind:this={emailForm}
      onsubmit={(event) => {
        event.preventDefault();
        void send();
      }}
    >
      <Field
        label={m.account_email()}
        labelHidden
        type="email"
        placeholder={m.account_email_placeholder()}
        autocomplete="email"
        spellcheck="false"
        autocapitalize="off"
        maxlength={254}
        bind:value={email}
        disabled={busy === "send"}
      />
      <Button type="submit" variant="primary" pending={busy === "send"} disabled={!emailValid}
        >{m.account_continue()}</Button
      >
    </form>
    <p class="terms">
      {m.account_terms()}
      <a
        href={TERMS_URL}
        onclick={(event) => {
          event.preventDefault();
          open(TERMS_URL);
        }}>{m.account_terms_link()}</a
      >
      ·
      <a
        href={PRIVACY_URL}
        onclick={(event) => {
          event.preventDefault();
          open(PRIVACY_URL);
        }}>{m.account_privacy_link()}</a
      >
    </p>
  {:else}
    <h2>{m.account_code_title()}</h2>
    <p class="body">{m.account_code_body({ email: pendingEmail })}</p>
    <form
      class="row"
      bind:this={codeForm}
      onsubmit={(event) => {
        event.preventDefault();
        void verify();
      }}
    >
      <Field
        class="code"
        label={m.account_code()}
        labelHidden
        inputmode="numeric"
        autocomplete="one-time-code"
        placeholder="000000"
        maxlength={11}
        spellcheck="false"
        value={code}
        oninput={(event) => typed(event.currentTarget.value)}
        disabled={busy === "verify"}
      />
      <Button
        type="submit"
        variant="primary"
        pending={busy === "verify"}
        disabled={codeDigits(code).length !== 6}>{m.account_continue()}</Button
      >
    </form>
    <div class="links">
      <Button
        variant="ghost"
        size="compact"
        disabled={resendIn > 0 || !!busy}
        pending={busy === "send"}
        onclick={() => void resend()}
        >{resendIn > 0
          ? m.account_code_resend_in({ seconds: resendIn })
          : m.account_code_resend()}</Button
      >
      <Button variant="ghost" size="compact" disabled={!!busy} onclick={() => void otherEmail()}
        >{m.account_code_other()}</Button
      >
    </div>
    {#if resent && !faultText}<p class="note" role="status">{m.account_code_sent()}</p>{/if}
  {/if}
  {#if faultText}<p class="note" data-tone="error" role="alert">{faultText}</p>{/if}
</div>

<style>
  .sign-in {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding-block: 8px 34px;
  }

  .mark {
    display: flex;
    margin-block-end: 18px;
  }

  h2 {
    margin: 0;
    color: var(--color-text);
    font-size: 18px;
    font-weight: 500;
    letter-spacing: -0.02em;
  }

  .body {
    max-inline-size: 450px;
    margin: 8px 0 20px;
    color: var(--color-muted);
    font-size: var(--text-body);
    line-height: 1.6;
  }

  .row {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    inline-size: min(100%, 420px);
  }

  .row :global(.ui-field) {
    flex: 1;
  }

  .row :global(.code input) {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.3em;
  }

  .links {
    display: flex;
    gap: 4px;
    margin-block-start: 10px;
    margin-inline-start: -10px;
  }

  .terms,
  .note {
    margin: 12px 0 0;
    color: var(--color-faint);
    font-size: var(--text-caption);
    line-height: 1.5;
  }

  .terms a {
    color: var(--color-muted);
    text-decoration: underline;
    text-decoration-color: var(--color-border);
    text-underline-offset: 2px;
  }

  .terms a:hover {
    color: var(--color-text);
  }

  .note {
    color: var(--color-muted);
    font-size: var(--text-label);
  }

  .note[data-tone="error"] {
    color: var(--color-danger);
  }
</style>
