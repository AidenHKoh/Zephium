import { expect, test } from "vitest";
import { searchProvider } from "../lib/search-provider";

test("a citation names OpenAI's search, or the model's own provider searching natively", () => {
  expect(searchProvider("open_ai", "gpt-6-luna")).toEqual({
    provider: "OpenAI web search",
    model: "gpt-6-luna",
  });
  expect(searchProvider("native", "anthropic/claude-haiku-4-5-20251001")).toEqual({
    provider: "Anthropic",
    model: "claude-haiku-4-5-20251001",
  });
  expect(searchProvider("native", "zephium/zr_quick")).toEqual({
    provider: "Zephium",
    model: "zr_quick",
  });
  expect(searchProvider("native", "mystery-model").provider).toBe("the model's provider");
});
