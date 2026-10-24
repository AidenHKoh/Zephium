import { describe, expect, it } from "vitest";
import { followUp } from "../lib/minds";

const ollama = { kind: "ollama", base: "http://localhost:11434/v1", models: 3 } as const;

describe("minds", () => {
  it("sends each choice to the settings that finish it", () => {
    expect(followUp("zephium", null)).toBe("account");
    expect(followUp("key", ollama)).toBe("models");
    expect(followUp("local", null)).toBe("models");
  });

  it("needs nothing after a local server it connected, or no choice", () => {
    expect(followUp("local", ollama)).toBeNull();
    expect(followUp(null, ollama)).toBeNull();
  });
});
