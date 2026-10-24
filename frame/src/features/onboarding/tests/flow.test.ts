import { describe, expect, it } from "vitest";
import { neighbour, position, STEPS, stepsFor } from "../lib/flow";

describe("flow", () => {
  it("walks welcome, then seven steps, ending ready", () => {
    expect(STEPS).toEqual([
      "welcome",
      "you",
      "import",
      "essentials",
      "launcher",
      "work",
      "ai",
      "ready",
    ]);
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

describe("flow without Work and AI", () => {
  it("skips Work and its models, and counts what is left", () => {
    expect(stepsFor(false)).not.toContain("work");
    expect(stepsFor(false)).not.toContain("ai");
    expect(position("ready", false)).toEqual({ at: 5, of: 5 });
    expect(neighbour("launcher", 1, false)).toBe("ready");
    expect(neighbour("ready", -1, false)).toBe("launcher");
  });
});
