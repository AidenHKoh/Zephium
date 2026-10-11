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
    expect(Array.from({ length: 19 }, (_, code) => String(code))).toContain(result);
    return result === "0";
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
    await survey(`${inbox}<aside><div role="progressbar">Loading a preview</div></aside>`),
  ).toBe(true);
  expect(
    await survey(
      '<main><div role="grid"><div role="row"></div><div role="row"></div></div></main>',
    ),
  ).toBe(false);
});

test("email and MFA transitions remain walls before or without editable controls", async () => {
  for (const html of [
    "<main><h1>Check your email</h1><p>We sent a sign-in link. Open your email to continue.</p></main>",
    "<main><h1>Enter your confirmation code</h1><p>Use the six-digit code from your email.</p><input inputmode='numeric' name='confirmation_code'><button>Continue</button></main>",
    "<main><h1>Approve sign-in on your phone</h1><p>Open your authenticator to continue.</p><a href='/help'>Help</a><a href='/terms'>Terms</a><a href='/privacy'>Privacy</a></main>",
    "<main><h1>Sign in</h1><p>Your sign-in form is loading.</p></main><footer><a href='/help'>Help</a><a href='/terms'>Terms</a><a href='/privacy'>Privacy</a></footer>",
    "<main><div role='progressbar'>Loading</div></main><footer><a href='/help'>Help</a><a href='/terms'>Terms</a><a href='/privacy'>Privacy</a></footer>",
  ])
    expect(await survey(html), html).toBe(false);
});

test("a verification screen clears only after it is replaced by account content", async () => {
  const waiting =
    "<main><h1>Enter the code from your email</h1><p>Use the message we sent to finish signing in.</p><div role='textbox' aria-label='Confirmation code' contenteditable='true'></div></main>";
  expect(await survey(waiting)).toBe(false);
  expect(await survey(content)).toBe(true);
});

test("unlabelled segmented verification controls and lower-level headings remain held", async () => {
  for (const html of [
    `<main><h4>Check your email</h4><p>Use the code we sent to continue.</p>${"<input type='text' maxlength='1' inputmode='numeric'>".repeat(6)}</main>`,
    `<main><h1>Continue to your workspace</h1><p>Complete the next step to open your account.</p><input type='text' name='code'><button>Continue</button></main>`,
    `<main><h5>Check your inbox</h5><p>Follow the link to continue to your workspace.</p></main>`,
  ])
    expect(await survey(html), html).toBe(false);
});

test("native HTML inbox items are account content without ARIA row attributes", async () => {
  const items =
    "<main><h1>Inbox</h1><ul><li>Prepare the release checklist</li><li>Review the project proposal</li><li>Read the integration notes</li></ul></main>";
  expect(await survey(items)).toBe(true);
  expect(
    await survey(
      `${items}<div role='dialog'><h2>Continue</h2><input name='code'><button>Continue</button></div>`,
    ),
  ).toBe(false);
});

test("native table rows can establish loaded inbox content", async () => {
  expect(
    await survey(
      "<main><h1>Inbox</h1><table><tbody><tr><td>Review the project proposal</td></tr><tr><td>Confirm the planning meeting</td></tr><tr><td>Read the engineering report</td></tr></tbody></table></main>",
    ),
  ).toBe(true);
});

test("an inbox of sibling card buttons clears without main or list landmarks", async () => {
  const cards =
    "<div aria-label='Main Content'><h1>Inbox</h1><div><button>Task: Prepare the project checklist</button><button>Task: Review the integration notes</button><button>Task: Read the planning report</button></div></div>";
  expect(await survey(cards)).toBe(true);
  const wrapped =
    "<div><h1>Inbox</h1><section>" +
    ["Prepare the project checklist", "Review the integration notes", "Read the planning report"]
      .map((label) => `<div><div><div><button>Task: ${label}</button></div></div></div>`)
      .join("") +
    "</section></div>";
  expect(await survey(wrapped)).toBe(true);
  expect(
    await survey(
      `${cards}<div role='dialog'><h2>Continue</h2><input name='code'><button>Continue</button></div>`,
    ),
  ).toBe(false);
  expect(
    await survey(
      "<h1>Welcome</h1><div><button>Continue with Google</button><button>Continue with Microsoft</button><button>Continue with Apple</button></div>",
    ),
  ).toBe(false);
  expect(
    await survey(
      "<h1>Welcome</h1><footer><button>Accessibility settings</button><button>Privacy preferences</button><button>Support information</button></footer>",
    ),
  ).toBe(false);
});

test("conversation search is not an identity entry field", async () => {
  for (const field of [
    "<input type='search' aria-label='Channel or user name'>",
    "<input role='searchbox' aria-label='Workspace search'>",
  ])
    expect(await survey(`${content}${field}`)).toBe(true);
  expect(await survey(`${content}<input type='text' aria-label='User name'>`)).toBe(false);
  for (const field of [
    "<input role='searchbox' type='email'>",
    "<input role='searchbox' type='password'>",
    "<input role='searchbox' autocomplete='one-time-code'>",
    "<form><input type='search' aria-label='User name'></form>",
  ])
    expect(await survey(`${content}${field}`)).toBe(false);
});

test("a multiline composer beside loaded conversation content can resume", async () => {
  const conversation =
    "<div><h1>Project channel</h1><div role='list'><div>Welcome to the project channel and its recent discussion.</div><button>Start from a template</button><button>Review channel details</button><button>Browse conversation history</button></div><div role='textbox' aria-multiline='true' contenteditable='true' aria-label='Message to project channel'></div></div>";
  expect(await survey(conversation)).toBe(true);
  for (const wall of [
    "<input type='password'>",
    "<input type='email'>",
    "<input autocomplete='one-time-code'>",
    "<div role='dialog'><h2>Continue</h2><input name='code'></div>",
  ])
    expect(await survey(conversation + wall)).toBe(false);
  expect(
    await survey(
      "<h1>Continue</h1><p>Enter the requested code to continue to your account.</p><textarea></textarea>",
    ),
  ).toBe(false);
  expect(
    await survey(
      "<h1>Continue</h1><form><div role='list'><button>Continue with Google</button><button>Continue with Microsoft</button><button>Continue with Apple</button></div><textarea></textarea></form>",
    ),
  ).toBe(false);
});

test("rich editor descendants remain one editable host and never become page content", async () => {
  const conversation =
    "<div><h1>Project channel</h1><div role='list'><div>Welcome to the project channel and its recent discussion.</div><button>Start from a template</button><button>Review channel details</button><button>Browse conversation history</button></div><div role='textbox' aria-multiline='true' contenteditable='true' aria-label='Message to project channel'><p><br></p></div></div>";
  expect(await survey(conversation)).toBe(true);
  expect(
    await survey(
      conversation.replace(
        "<p><br></p>",
        "<h2>Sign in to your account</h2><p>Draft text stays inside the editor.</p>",
      ),
    ),
  ).toBe(true);
  expect(
    await survey(
      conversation.replace(
        "<p><br></p>",
        "<span contenteditable='false'><h2>Sign in to your account</h2></span>",
      ),
    ),
  ).toBe(true);
  for (const control of [
    "<input type='password'>",
    "<input autocomplete='one-time-code'>",
    "<iframe title='hCaptcha challenge' width='300'></iframe>",
  ]) {
    expect(await survey(conversation.replace("<p><br></p>", control))).toBe(false);
  }
});

test("a short accessible list name does not hide its populated conversation", async () => {
  const conversation =
    "<div><h1>Project channel</h1><div role='list' aria-label='Project channel'><div>Welcome to the project channel and its recent discussion.</div><button>Start from a template</button><button>Review channel details</button><button>Browse conversation history</button></div><div role='textbox' aria-multiline='true' contenteditable='true' aria-label='Message to project channel'></div></div>";
  expect(await survey(conversation)).toBe(true);
  expect(
    await survey(
      conversation.replace(
        "<div>Welcome to the project channel and its recent discussion.</div><button>Start from a template</button><button>Review channel details</button><button>Browse conversation history</button>",
        "<div></div>",
      ),
    ),
  ).toBe(false);
});
