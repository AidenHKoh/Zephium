import "$styles/global.css";
import { expect, test } from "vitest";
import { render } from "vitest-browser-svelte";
import { page } from "vitest/browser";
import AttachPanelStage from "./AttachPanelStage.svelte";

test("the panel is as tall as its kind needs and grows and shrinks to it, with the switcher under it", async () => {
  await page.viewport(800, 800);
  const screen = await render(AttachPanelStage);
  const box = (selector: string) => screen.container.querySelector<HTMLElement>(selector)!;
  const height = () => box(".morph").getBoundingClientRect().height;
  await expect.poll(height).toBeGreaterThan(250);
  const tall = height();

  await page.getByRole("button", { name: "Links" }).click();
  // Both kinds are in the place while one slides out and the next in.
  await expect.poll(() => screen.container.querySelectorAll(".morph .view").length).toBe(2);
  await expect.poll(() => screen.container.querySelectorAll(".morph .view").length).toBe(1);
  await expect.poll(height).toBeLessThan(tall - 60);
  await expect.element(page.getByPlaceholder("Paste a link")).toBeVisible();

  // The kinds' hover and chosen plates follow the capsule.
  for (const segment of screen.container.querySelectorAll<HTMLElement>(".segment"))
    expect(getComputedStyle(segment).borderTopLeftRadius).not.toBe("0px");

  // Not a fixed height: the place settles on the new kind's own, the switcher under it.
  const short = height();
  await page.getByRole("button", { name: "Documents" }).click();
  await expect.poll(height).toBeGreaterThan(short + 60);
  const place = box(".morph").getBoundingClientRect();
  expect(box(".segments").getBoundingClientRect().top).toBeGreaterThanOrEqual(place.bottom);
  await screen.unmount();
});
