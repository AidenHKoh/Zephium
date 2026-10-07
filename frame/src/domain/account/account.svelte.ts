/**
 * The optional Zephium account, one per install. Rust owns the session, the
 * vault and every outcome; this mirrors its last answer. Codes and addresses
 * pass through once and are not kept here.
 */
import {
  commands,
  type AccountFaultV1,
  type AccountPurchaseV1,
  type AccountReplyV1,
  type AccountSessionV1,
  type AccountV1,
} from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";
import { observe } from "$shared/lib/observe";

const TIMEOUT = 15_000;
/** Signing out tells the server first, and must not hang on it. */
const SIGN_OUT_TIMEOUT = 30_000;

type AccountAction =
  | "send"
  | "verify"
  | "sign_out"
  | "refresh"
  | "buy"
  | "manage"
  | "revoke"
  | "server"
  | "deletion_code"
  | "delete";

type AccountFault = {
  action: AccountAction;
  fault: AccountFaultV1;
  /** With `rate_limited`, seconds until another try. */
  retryAfter: number | null;
};

let current = $state.raw<AccountV1 | null>(null);
let unreachable = $state(false);
let busy = $state<AccountAction | null>(null);
let lastFault = $state.raw<AccountFault | null>(null);
let users = 0;
let stop: (() => void) | null = null;
let generation = 0;
let reads = 0;

export const account = () => current;
export const unavailable = () => unreachable || (current !== null && !current.available);
export const pending = () => busy;
export const fault = () => lastFault;

function adopt(reply: AccountReplyV1) {
  if (reply.account.version !== 1) {
    unreachable = true;
    return;
  }
  unreachable = false;
  current = reply.account;
}

async function read() {
  const sequence = ++reads;
  const started = generation;
  const response = await observe(
    Promise.resolve().then(() => commands.accountGet()),
    TIMEOUT,
  );
  if (started !== generation || sequence !== reads) return;
  if (response.state !== "received") {
    if (!current) unreachable = true;
    return;
  }
  adopt(response.value);
}

/**
 * Starts mirroring; each user calls `release` when done. The listener is in
 * place before the first read, so a change in between is never lost.
 */
export async function retain() {
  users += 1;
  if (users > 1) return;
  const started = ++generation;
  const unlisten = await events.accountChanged.listen(() => void read());
  if (started !== generation || users === 0) {
    unlisten();
    return;
  }
  stop = unlisten;
  await read();
}

export function release() {
  users = Math.max(0, users - 1);
  if (users > 0) return;
  generation += 1;
  stop?.();
  stop = null;
  busy = null;
  lastFault = null;
}

async function act(
  action: AccountAction,
  request: () => Promise<AccountReplyV1>,
  timeout = TIMEOUT,
): Promise<boolean> {
  if (busy) return false;
  busy = action;
  lastFault = null;
  reads += 1;
  const started = generation;
  const response = await observe(Promise.resolve().then(request), timeout);
  if (started !== generation) return false;
  busy = null;
  if (response.state !== "received") {
    lastFault = { action, fault: "offline", retryAfter: null };
    return false;
  }
  adopt(response.value);
  const failed = response.value.fault;
  if (failed) lastFault = { action, fault: failed, retryAfter: response.value.retry_after };
  return !failed;
}

export function clearFault() {
  lastFault = null;
}

export const sendCode = (email: string) => act("send", () => commands.accountSendCode(email));
export const verify = (code: string) => act("verify", () => commands.accountVerify(code));
export const buy = (purchase: AccountPurchaseV1) => act("buy", () => commands.accountBuy(purchase));
export const manage = () => act("manage", () => commands.accountManage());
export const setServer = (server: string | null) =>
  act("server", () => commands.accountSetServer(server));
export const sendDeletionCode = () => act("deletion_code", () => commands.accountDeletionCode());
export const deleteAccount = (code: string) => act("delete", () => commands.accountDelete(code));
export const signOut = () => act("sign_out", () => commands.accountSignOut(), SIGN_OUT_TIMEOUT);
export const revoke = (id: string) => act("revoke", () => commands.accountRevokeSession(id));

/** Goes back to entering an address; nothing is sent. */
export async function cancelCode() {
  lastFault = null;
  const response = await observe(
    Promise.resolve().then(() => commands.accountCancelCode()),
    TIMEOUT,
  );
  if (response.state === "received") adopt(response.value);
}

export async function sessions(): Promise<AccountSessionV1[] | AccountFaultV1> {
  const response = await observe(
    Promise.resolve().then(() => commands.accountSessions()),
    TIMEOUT,
  );
  if (response.state !== "received") return "offline";
  return response.value.fault ?? response.value.sessions;
}
