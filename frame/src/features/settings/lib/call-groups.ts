import type { AccountRequestV1 } from "$shared/ipc/bindings";
import { requestStanding, sumUnits } from "$domain/account";

/** The calls made for one thing: a Work task, voice, or none this computer knows. */
export type CallGroup = {
  /** Stable while pages load: a task's profile and id, `voice`, or `other`. */
  key: string;
  kind: "task" | "voice" | "other";
  /** The task's owner and id, to look its title up; only with `task`. */
  task: { profile: string; work: string } | null;
  /** Newest first, as the service lists them. */
  calls: AccountRequestV1[];
  /** When its newest call began. */
  latest: string | null;
  /** Units charged by settled calls. Calls still being confirmed add nothing. */
  total: string;
  /** Calls still running or being confirmed. */
  unsettled: number;
};

/**
 * Groups history by what each call was made for, using this computer's own
 * record of it. Groups keep the order their newest call gives them.
 */
export function groupCalls(requests: readonly AccountRequestV1[]): CallGroup[] {
  const groups = new Map<string, CallGroup>();
  for (const call of requests) {
    const source = call.source;
    const key =
      source?.kind === "task" ? `task:${source.profile}:${source.work}` : (source?.kind ?? "other");
    let group = groups.get(key);
    if (!group) {
      group = {
        key,
        kind: source?.kind ?? "other",
        task: source?.kind === "task" ? { profile: source.profile, work: source.work } : null,
        calls: [],
        latest: call.started_at,
        total: "0",
        unsettled: 0,
      };
      groups.set(key, group);
    }
    group.calls.push(call);
  }
  for (const group of groups.values()) {
    group.total = sumUnits(
      group.calls.filter((call) => call.state === "settled").map((call) => call.charged),
    );
    group.unsettled = group.calls.filter((call) => {
      const standing = requestStanding(call.state);
      return standing === "checking" || standing === "running";
    }).length;
  }
  return [...groups.values()];
}
