import { describe, expect, it } from "vitest";
import { searchSections } from "../lib/settings-model";

describe("settings discovery", () => {
  it("finds nested preferences by their localized vocabulary", () => {
    expect(searchSections("languages").map((section) => section.id)).toContain("languages");
    expect(searchSections("motion").map((section) => section.id)).toContain("appearance");
    expect(searchSections("download").map((section) => section.id)).toContain("general");
  });
  it("matches every search word and distinguishes no results", () => {
    expect(searchSections("  ACCENT color  ").map((section) => section.id)).toEqual(["appearance"]);
    expect(searchSections("accent downloads")).toEqual([]);
    expect(searchSections("unfindablephrase")).toEqual([]);
  });
});

it("finds the agent destinations without exposing unimplemented controls", async () => {
  const { searchSettings, searchSections, sections } = await import("../lib/settings-model");
  expect(sections.some((section) => String(section.id) === "plugins")).toBe(false);
  expect(searchSections("models").some((section) => section.id === "ai")).toBe(true);
  expect(searchSections("site access").some((section) => section.id === "sites")).toBe(true);
  expect(searchSections("MCP").some((section) => section.id === "mcp")).toBe(true);
  for (const query of ["API key", "Jev", "TypeSafe"]) {
    expect(searchSettings(query).map(({ section, target }) => ({ section, target }))).toEqual([
      { section: "ai", target: "work.decisions.key" },
    ]);
  }
  expect(searchSettings("Decisions").some((result) => result.section === "work")).toBe(false);
  expect(searchSettings("reset settings").some((result) => result.target === "about.reset")).toBe(
    true,
  );
});

it("offers Account through the identity card and search, outside the groups", async () => {
  const { groups, sections, searchSettings } = await import("../lib/settings-model");
  const account = sections.find((section) => String(section.id) === "account");
  expect(account).toBeDefined();
  expect(groups.some((group) => String(group.id) === account!.group)).toBe(false);
  expect(searchSettings("account").some((result) => String(result.section) === "account")).toBe(
    true,
  );
});

it("groups sections as Browser, Work and App with stable ids", async () => {
  const { groups, sections } = await import("../lib/settings-model");
  const listed = (group: string) =>
    sections.filter((section) => section.group === group).map((section) => section.id);
  expect(groups.map((group) => group.id)).toEqual(["browser", "work", "app"]);
  expect(listed("browser").filter((id) => id !== "profiles")).toEqual([
    "general",
    "appearance",
    "tabs",
    "newtab",
    "search",
    "privacy",
    "focus",
    "performance",
  ]);
  expect(listed("work")).toEqual(["work", "ai", "skills", "memory", "sites", "mcp"]);
  expect(listed("app")).toEqual(["shortcuts", "languages", "about"]);
});

it("finds every real tab option", async () => {
  const { searchSettings } = await import("../lib/settings-model");
  for (const [query, target] of [
    ["archive", "tabs.archive"],
    ["other apps", "tabs.external"],
    ["close buttons", "tabs.close-button"],
    ["login", "general.login"],
  ]) {
    expect(searchSettings(query!).some((result) => result.target === target)).toBe(true);
  }
});
