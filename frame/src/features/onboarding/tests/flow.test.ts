import { describe, expect, it } from "vitest";
import { neighbour, position, STEPS } from "../lib/flow";

describe("flow", () => {
  it("walks welcome, then seven steps, ending ready", () => {
    expect(STEPS).toEqual(["welcome", "you", "import", "essentials", "launcher", "work", "ai", "ready"]);
  });

  it("counts from the first real step, welcome being the door", () => {
    expect(position("you")).toEqual({ at: 1, of: 7 });
    expect(position("ready")).toEqual({ at: 7, of: 7 });
  });

  it("never steps back through the door or past the end", () => {
    expect(neighbour("you", -1)).toBeUndefined();
    expect(neighbour("you", 1)).toBe("import");
    expect(neighbour("ready", 1)).toBeUndefined();
  });
});
