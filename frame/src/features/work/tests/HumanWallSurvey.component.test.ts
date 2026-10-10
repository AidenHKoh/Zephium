import { expect, inject, test } from "vitest";

declare module "vitest" {
  export interface ProvidedContext {
    humanWallSurvey: string;
  }
}

const content =
  "<h1>Your workspace</h1><p>Here are the recent messages from your project team.</p>";
const links =
  "<main><a href='/a'>First account</a><a href='/b'>Second account</a><a href='/c'>Third account</a></main>";

async function survey(html: string): Promise<boolean> {
  const frame = document.createElement("iframe");
  frame.width = "640";
  frame.height = "480";
  const loaded = new Promise<void>((resolve) =>
    frame.addEventListener("load", () => resolve(), { once: true }),
  );
  frame.srcdoc = `${html}<script>document.addEventListener("DOMContentLoaded", () => { const result = ${inject("humanWallSurvey")}\ndocument.documentElement.dataset.wallClear = String(result); });</script>`;
  document.body.append(frame);
  try {
    await loaded;
    const result = frame.contentDocument?.documentElement.dataset.wallClear;
    expect(["true", "false"]).toContain(result);
    return result === "true";
  } finally {
    frame.remove();
  }
}

test("a loaded shell, credential field, chooser or check cannot prove wall clearance", async () => {
  for (const html of [
    "<main><div role='progressbar'>Loading</div></main>",
    `${content}<input type='password' value='never-return-this-secret'>`,
    `${content}<input autocomplete='one-time-code'>`,
    "<h1>Find your workspace</h1><input placeholder='Workspace URL'><button>Continue</button>",
    `<h1>Choose an account</h1>${links}`,
    `<h1>Select your workspace</h1>${links}`,
    `<h1>Verify you are human</h1>${links}`,
    `<main aria-label="Sign in"><input type="email"><button>Continue</button>${links}</main>`,
    `<main aria-label="Choose an account">${links}</main>`,
    `<main><input autocomplete="username"><button>Continue</button>${links}</main>`,
    `<main><h1>Connectez-vous</h1><p>Identifiez-vous avec votre adresse professionnelle pour continuer.</p><form><input type="email"><button>Continuer</button></form></main>`,
    `${content}<iframe title='hCaptcha challenge' width='300'></iframe>`,
  ])
    expect(await survey(html), html).toBe(false);
});

test("visible app content clears a wall, while optional sign-in and hidden controls do not block it", async () => {
  expect(await survey(content)).toBe(true);
  expect(await survey(`${content}<button>Sign in</button>`)).toBe(true);
  expect(await survey(`${content}<input type='password' hidden>`)).toBe(true);
  expect(
    await survey(`${content}<form><input type="email"><button>Subscribe</button></form>`),
  ).toBe(true);
});

test("an accessible inbox can resume without paragraphs or native anchor links", async () => {
  const inbox = `<section role="main"><h2>Inbox</h2><div role="grid">
    <div role="row"><span role="link">Project update from the engineering team</span></div>
    <div role="row"><span role="link">Meeting notes for next week's review</span></div>
    <div role="row"><span role="link">Your saved research draft is available</span></div>
  </div></section>`;
  expect(await survey(inbox)).toBe(true);
  expect(
    await survey(
      '<main><div role="grid"><div role="row"></div><div role="row"></div></div></main>',
    ),
  ).toBe(false);
});
