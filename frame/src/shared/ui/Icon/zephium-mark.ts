import type { IconSvgElement } from "@hugeicons/svelte";

/** Zephium's own symbol: two opposed hooks around a diamond, as on the app icon. */
export const ZephiumMark: IconSvgElement = [
  [
    "path",
    { d: "M20 6H9C6.8 6 5 7.8 5 10", strokeLinecap: "round", strokeLinejoin: "round", key: "0" },
  ],
  [
    "path",
    {
      d: "M4 18H15C17.2 18 19 16.2 19 14",
      strokeLinecap: "round",
      strokeLinejoin: "round",
      key: "1",
    },
  ],
  ["path", { d: "M12 9.5L14.5 12L12 14.5L9.5 12Z", strokeLinejoin: "round", key: "2" }],
];
