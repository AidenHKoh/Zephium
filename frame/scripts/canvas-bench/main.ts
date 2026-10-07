/**
 * The Work canvas built as the app ships it, over a seeded scene and a fake
 * native side; `run.ts` drives it through `window.bench` in the system WebKit.
 */
import "$styles/global.css";
import { flushSync, mount, unmount } from "svelte";
import WorkCanvas from "../../src/features/work/components/WorkCanvas.svelte";
import { wakeupCounter } from "../../src/shared/testing/idle";
import {
  drawn,
  drive,
  scaleCanvas,
  settle,
  sweep,
  type ScaleMix,
} from "../../src/features/work/tests/scale-fixtures";

const counter = wakeupCounter();
const pictures = new Map<string, string>();
(globalThis as { __benchPictures?: Map<string, string> }).__benchPictures = pictures;

let callbackId = 0;
const answers: Record<string, unknown> = {
  favicon_probe: true,
  work_release_memory: true,
};
(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
  invoke: async (command: string) => {
    if (command === "plugin:event|listen") return ++callbackId;
    if (command in answers) return answers[command];
    throw new Error(`unmocked ${command}`);
  },
  transformCallback: () => ++callbackId,
  unregisterCallback() {},
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
};

const host = document.getElementById("root")!;
let screen: Record<string, unknown> | null = null;
let center: (id: string) => void = () => {};
let count = 0;

const bench = {
  async open({ items: n, zoom, mix }: { items: number; zoom: number; mix?: ScaleMix }) {
    count = n;
    const canvas = scaleCanvas(n, (index) => `/photos/photo-${index}.jpg`, mix);
    pictures.clear();
    for (const [digest, url] of canvas.pictures) pictures.set(digest, url);
    const opened = performance.now();
    screen = mount(WorkCanvas, {
      target: host,
      props: {
        items: canvas.items,
        links: [],
        authoritative: new Set<string>(),
        initialView: { positions: canvas.positions, viewport: { x: 40, y: 40, zoom } },
        oninspect: () => {},
        expose: (api: { center: (id: string) => void }) => (center = api.center),
      },
    });
    flushSync();
    const mounted = performance.now() - opened;
    await settle(host);
    return { mountMs: Math.round(mounted), ...drawn(host) };
  },
  idle: () => counter.rest(2000),
  css(text: string) {
    document.getElementById("bench-css")?.remove();
    if (text)
      document.head.insertAdjacentHTML("beforeend", `<style id="bench-css">${text}</style>`);
  },
  pan: () => drive(host.querySelector(".svelte-flow__pane")!, 180, false),
  pinch: () => drive(host.querySelector(".svelte-flow__pane")!, 120, true),
  async sweep() {
    await sweep(center, count);
    await settle(host);
    return drawn(host);
  },
  async back() {
    center("item-0");
    await settle(host);
    return drawn(host);
  },
  close() {
    if (screen) void unmount(screen);
    screen = null;
  },
};
(window as unknown as { bench: typeof bench }).bench = bench;
