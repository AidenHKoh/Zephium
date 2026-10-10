import { describe, expect, it } from "vitest";
import { comparable } from "../components/objects/centre";
import { pickCandidates } from "../lib/link-media";
import type { PicksView, PickView } from "../lib/board/types";

const pick = (name: string, extra: Partial<PickView> = {}): PickView => ({
  name,
  facts: [],
  tags: [],
  recommended: false,
  ...extra,
});
const set = (facet: PicksView["facet"], items: PickView[]): PicksView => ({
  id: "p",
  kind: "picks",
  facet,
  items,
});

describe("picks shape", () => {
  it("offers Compare only for things that line up", () => {
    const priced = (name: string) => pick(name, { price: { display: "$1" } });
    expect(comparable(set("product", [priced("A"), priced("B")]))).toBe(true);
    expect(comparable(set("video", [priced("A"), priced("B")]))).toBe(false);
    expect(comparable(set("other", [pick("PR #1"), pick("Author and date")]))).toBe(false);
    const fact = (label: string) => ({ label, value: "x", kind: "text" as const });
    const shared = [fact("VRAM"), fact("Condition")];
    expect(
      comparable(set("other", [pick("A", { facts: shared }), pick("B", { facts: shared })])),
    ).toBe(true);
  });

  it("falls back to a linked video's thumbnail", () => {
    expect(pickCandidates({ url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ" })).toEqual([
      "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
    ]);
    expect(
      pickCandidates({ image_candidates: ["https://a.test/x.jpg"], url: "https://youtu.be/x" }),
    ).toEqual(["https://a.test/x.jpg"]);
    expect(pickCandidates({ url: "https://example.com" })).toEqual([]);
  });
});
