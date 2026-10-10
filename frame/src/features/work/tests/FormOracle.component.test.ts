import { expect, inject, test } from "vitest";

declare module "vitest" {
  export interface ProvidedContext {
    formOracleRuntime: string;
  }
}

type Evidence = { version: number; checks: (boolean | null)[] };
async function observe(task: string, href: string, body: string): Promise<Evidence> {
  const source = inject("formOracleRuntime").replace("__FORM_ORACLE_CASE__", JSON.stringify(task));
  const frame = document.createElement("iframe");
  const loaded = new Promise<void>((done) =>
    frame.addEventListener("load", () => done(), { once: true }),
  );
  // Actual browser DOM predicates; substitute only the native-owned URL operand.
  // Native origin/document ownership is independently checked by the host gate.
  frame.srcdoc = `${body}<script>
    const source = ${JSON.stringify(source)};
    const result = Function("location", "return (" + source + "\\n)")({href:${JSON.stringify(href)}});
    document.documentElement.dataset.evidence = result;
  </script>`;
  document.body.append(frame);
  try {
    await loaded;
    return JSON.parse(frame.contentDocument!.documentElement.dataset.evidence!) as Evidence;
  } finally {
    frame.remove();
  }
}

const httpbin =
  '<input name="custname" value="Eval Person"><input name="custtel"><input name="custemail"><input type="time" name="delivery" value="18:30"><input type="radio" name="size" value="small"><input type="radio" name="size" value="medium" checked><input type="radio" name="size" value="large"><input type="checkbox" name="topping" value="bacon"><input type="checkbox" name="topping" value="cheese"><input type="checkbox" name="topping" value="onion"><input type="checkbox" name="topping" value="mushroom" checked>';
test("fixed draft predicate checks every requested field and unchanged choice", async () => {
  const result = await observe("form-httpbin-draft", "https://httpbin.org/forms/post", httpbin);
  expect(result).toEqual({ version: 1, checks: Array(11).fill(true) });
  const wrong = await observe(
    "form-httpbin-draft",
    "https://httpbin.org/forms/post",
    httpbin.replace('value="bacon"', 'value="bacon" checked'),
  );
  expect(wrong.checks.filter((check) => check === false)).toHaveLength(1);
});
test("foreign and ambiguous pages cannot provide a passing predicate", async () => {
  expect(
    (await observe("form-httpbin-draft", "https://httpbin.org/private", httpbin)).checks,
  ).toEqual([null]);
  const ambiguous = await observe(
    "form-httpbin-draft",
    "https://httpbin.org/forms/post",
    `${httpbin}<input name="custname" value="Eval Person">`,
  );
  expect(ambiguous.checks[0]).toBeNull();
});
test("selected option value and visible label must both match", async () => {
  const body =
    '<input id="my-text-id" value="Eval Sample"><select name="my-select"><option value="1">One</option><option value="2" selected>Two</option></select><input type="checkbox" id="my-check-2">';
  expect(
    (
      await observe(
        "form-selenium-controls",
        "https://www.selenium.dev/selenium/web/web-form.html",
        body,
      )
    ).checks,
  ).toEqual([true, true, true]);
  expect(
    (
      await observe(
        "form-selenium-controls",
        "https://www.selenium.dev/selenium/web/web-form.html",
        body.replace(">Two<", ">Wrong<"),
      )
    ).checks,
  ).toEqual([true, false, true]);
});
test("final numeric and empty values never fabricate an action sequence", async () => {
  expect(
    (
      await observe(
        "form-heroku-numeric",
        "https://the-internet.herokuapp.com/inputs",
        '<input type="number" value="18">',
      )
    ).checks,
  ).toEqual([true, null]);
  expect(
    (
      await observe(
        "form-expand-inputs",
        "https://practice.expandtesting.com/inputs",
        '<input id="input-number" type="number" value="42"><input id="input-text">',
      )
    ).checks,
  ).toEqual([true, true, null]);
});
test("control values are reduced to booleans and never exported", async () => {
  const result = await observe(
    "form-expand-inputs",
    "https://practice.expandtesting.com/inputs",
    '<input id="input-number" value="42"><input id="input-text" value="synthetic-private-marker">',
  );
  expect(result.checks).toEqual([true, false, null]);
  expect(JSON.stringify(result)).not.toContain("synthetic-private-marker");
});
test("checkbox order and count are exact, date text stays unverified", async () => {
  expect(
    (
      await observe(
        "form-heroku-checkboxes",
        "https://the-internet.herokuapp.com/checkboxes",
        '<form id="checkboxes"><input type="checkbox" checked><input type="checkbox"></form>',
      )
    ).checks,
  ).toEqual([true, true]);
  expect(
    (
      await observe(
        "form-heroku-checkboxes",
        "https://the-internet.herokuapp.com/checkboxes",
        '<form id="checkboxes"><input type="checkbox" checked></form>',
      )
    ).checks,
  ).toEqual([null, null]);
  expect(
    (
      await observe(
        "form-selenium-date",
        "https://www.selenium.dev/selenium/web/web-form.html",
        '<input name="my-date" value="11/21/2026"><input type="range" name="my-range" min="0" max="10" value="5">',
      )
    ).checks,
  ).toEqual([null, true]);
});
