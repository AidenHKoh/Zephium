import {
  commands,
  type PageAssistChangedV1,
  type PageAssistIntentV1,
  type PageAssistStatusV1,
  type PageAssistSuggestionV1,
  type PageStyleV1,
} from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";
import { suggestionIntent, suggestionLabel } from "./suggestions";
import { EMPTY, admit, lastTurn, reconcile, working, type Thread } from "./thread-model";

/** A question may be this long; the core holds the same bound. */
export const ASK_LIMIT = 2048;

/** Availabilities that mean the page is not asked about, and nothing was sent. */
export type Refusal = Exclude<
  PageAssistStatusV1["availability"],
  "ready" | "loading" | "unavailable"
>;

/**
 * The panel's view of one tab's thread. The core owns the thread; this follows
 * the active tab, rebuilds from the core's snapshot, and admits each live event
 * once, by (turn, seq). It lets go of everything when the tab changes: the
 * frame never holds more than the thread in front of the person.
 */
export class AssistSession {
  tab = $state<string | null>(null);
  status = $state.raw<PageAssistStatusV1 | null>(null);
  thread = $state.raw<Thread>(EMPTY);
  /** The styles the core holds for this tab's site, for the badge of an applied one. */
  styles = $state.raw<PageStyleV1[]>([]);
  /** The first read of this tab has not come back yet. */
  loading = $state(false);
  /** The core could not be reached for this tab. */
  unavailable = $state(false);

  private generation = 0;
  private queue: PageAssistChangedV1[] | null = null;
  private stopListening: (() => void) | null = null;
  private active = false;

  constructor(readonly profile: string) {}

  get busy(): boolean {
    return working(this.thread);
  }

  get site(): string | null {
    return this.status?.site ?? null;
  }

  get refusal(): Refusal | null {
    const availability = this.status?.availability;
    return availability &&
      availability !== "ready" &&
      availability !== "loading" &&
      availability !== "unavailable"
      ? availability
      : null;
  }

  /** Listening starts before anything is asked for, so no event falls between. */
  async start() {
    if (this.active) return;
    this.active = true;
    const stop = await events.pageAssistChanged.listen(({ payload }) => this.receive(payload));
    if (!this.active) {
      stop();
      return;
    }
    this.stopListening = stop;
  }

  /** The tab the panel is about. Anything held for another tab is dropped. */
  follow(tab: string | null) {
    if (tab === this.tab) return;
    this.generation++;
    this.tab = tab;
    this.thread = EMPTY;
    this.status = null;
    this.styles = [];
    this.unavailable = false;
    this.queue = null;
    if (tab === null) {
      this.loading = false;
      return;
    }
    this.loading = true;
    void this.read(tab, this.generation, true);
  }

  /** Asks the core again for what is true now, without losing the thread. */
  refresh() {
    const tab = this.tab;
    if (tab === null) return;
    void this.read(tab, this.generation, false);
  }

  private async read(tab: string, generation: number, first: boolean) {
    if (first) this.queue = [];
    let status: PageAssistStatusV1;
    try {
      status = await commands.pageAssistStatus(tab);
    } catch {
      if (generation !== this.generation) return;
      this.unavailable = true;
      this.loading = false;
      this.flush();
      return;
    }
    if (generation !== this.generation || status.tab !== tab) return;
    this.unavailable = status.availability === "unavailable";
    this.status = status;
    this.thread = reconcile(first ? EMPTY : this.thread, status.entries);
    this.loading = false;
    this.flush();
    if (status.styled) void this.readStyles(tab, generation);
  }

  private async readStyles(tab: string, generation: number) {
    try {
      const view = await commands.pageStyles(tab);
      if (generation === this.generation) this.styles = view.styles;
    } catch {
      // The badge keeps what it last showed.
    }
  }

  private flush() {
    const queued = this.queue ?? [];
    this.queue = null;
    for (const event of queued) this.receive(event);
  }

  private receive(event: PageAssistChangedV1) {
    if (event.tab !== this.tab) return;
    if (this.queue) {
      this.queue.push(event);
      return;
    }
    this.thread = admit(this.thread, event);
    const kind = event.kind;
    if (kind.type === "invalidated") this.refresh();
    if (kind.type === "style" && kind.style.state === "applied") this.refresh();
  }

  /** True when the core admitted the question. Nothing is shown until its events say so. */
  async ask(
    text: string,
    intent: PageAssistIntentV1 = "ask",
    suggestion: PageAssistSuggestionV1 | null = null,
  ): Promise<boolean> {
    const tab = this.tab;
    const question = text.trim().slice(0, ASK_LIMIT);
    if (tab === null || !question || this.busy || this.refusal || this.loading) return false;
    try {
      const reply = await commands.pageAssistAsk({ tab, text: question, intent, suggestion });
      if (reply.turn === null) return false;
      // The echo of the question comes with the next read; the answer's events need no wait.
      this.refresh();
      return true;
    } catch {
      return false;
    }
  }

  suggest(suggestion: PageAssistSuggestionV1) {
    return this.ask(suggestionLabel(suggestion), suggestionIntent(suggestion), suggestion);
  }

  /** Asks the last question again, as it was asked. */
  async retry(): Promise<boolean> {
    const last = lastTurn(this.thread);
    return last ? this.ask(last.question, last.intent, last.suggestion) : false;
  }

  stop() {
    if (this.tab) void commands.pageAssistStop(this.tab).catch(() => {});
  }

  cite(turn: number, n: number) {
    const found = this.thread.items.find((item) => item.type === "turn" && item.turn === turn);
    if (!this.tab || !found || (found.type === "turn" && found.inert)) return;
    void commands.pageAssistCite(this.tab, turn, n).catch(() => {});
  }

  decide(turn: number, approve: boolean) {
    if (this.tab)
      void commands
        .pageAssistDecide(this.tab, turn, approve ? "approve" : "decline")
        .catch(() => {});
  }

  resume(turn: number) {
    if (this.tab) void commands.pageAssistResume(this.tab, turn).catch(() => {});
  }

  handoff(turn: number) {
    if (this.tab) void commands.pageAssistHandoff(this.tab, turn).catch(() => {});
  }

  /** Turns an applied style off or back on for its site. */
  async toggleStyle(id: string, enabled: boolean) {
    const tab = this.tab;
    if (!tab) return;
    try {
      const view = await commands.pageStyleToggle(tab, id, enabled);
      if (tab === this.tab) this.styles = view.styles;
    } catch {
      // The badge keeps what it last showed.
    }
  }

  async removeStyle(id: string) {
    const tab = this.tab;
    if (!tab) return;
    try {
      const view = await commands.pageStyleRemove(tab, id);
      if (tab === this.tab) this.styles = view.styles;
    } catch {
      // The badge keeps what it last showed.
    }
  }

  async clear() {
    const tab = this.tab;
    if (!tab) return;
    try {
      if (await commands.pageAssistClear(tab)) {
        if (tab === this.tab) this.thread = EMPTY;
        this.refresh();
      }
    } catch {
      // The thread stays as the core still holds it.
    }
  }

  dispose() {
    this.active = false;
    this.generation++;
    this.stopListening?.();
    this.stopListening = null;
    this.queue = null;
    this.tab = null;
    this.thread = EMPTY;
    this.status = null;
  }
}
