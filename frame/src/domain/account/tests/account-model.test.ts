import { describe, expect, it } from "vitest";
import { initials, offerKey, planName, priceLabel, shortDate, usageTone } from "../account-model";

describe("account model", () => {
  it("draws initials from a name, else from the address", () => {
    expect(initials("ada@example.com", "Ada Lovelace")).toBe("AL");
    expect(initials("ada@example.com", "  ada ")).toBe("A");
    expect(initials("_42x@example.com")).toBe("4");
    expect(initials("@example.com")).toBe("");
    expect(initials(null)).toBe("");
  });

  it("names plans the way people read them", () => {
    expect(planName("pro")).toBe("Pro");
    expect(planName("team_plus")).toBe("Team Plus");
    expect(planName(" ")).toBeNull();
    expect(planName(null)).toBeNull();
  });

  it("shows only prices it can state exactly", () => {
    expect(priceLabel({ amount_minor: 2000, currency: "usd" }, "en-US")).toBe("$20");
    expect(priceLabel({ amount_minor: 1999, currency: "EUR" }, "en-US")).toBe("€19.99");
    expect(priceLabel({ amount_minor: 500, currency: "JPY" }, "en-US")).toBe("¥500");
    expect(priceLabel({ amount_minor: 100, currency: "dollars" }, "en-US")).toBeNull();
    expect(priceLabel(null)).toBeNull();
  });

  it("states the renewal day, with the year only when it differs", () => {
    const now = new Date("2026-10-07T12:00:00Z");
    expect(shortDate("2026-10-14T00:00:00Z", now, "en-US")).toMatch(/Oct 1[34]/u);
    expect(shortDate("2027-01-01T12:00:00Z", now, "en-US")).toContain("2027");
    expect(shortDate("not a date", now)).toBeNull();
    expect(shortDate(null, now)).toBeNull();
  });

  it("tones the meter as the allowance runs low", () => {
    expect(usageTone(null)).toBe("calm");
    expect(usageTone(79)).toBe("calm");
    expect(usageTone(80)).toBe("high");
    expect(usageTone(100)).toBe("full");
  });

  it("keys offers by what they buy", () => {
    expect(
      offerKey({
        kind: "plan",
        plan: "pro",
        interval: "yearly",
        name: null,
        price: null,
        features: [],
      }),
    ).toBe("plan:pro:yearly");
    expect(
      offerKey({
        kind: "top_up",
        id: "s",
        name: null,
        units: null,
        price: null,
        expires_days: null,
      }),
    ).toBe("topup:s");
  });
});
