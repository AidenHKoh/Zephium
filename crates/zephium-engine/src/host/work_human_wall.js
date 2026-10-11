// A fixed, read-only, content-free survey while the person owns this page.
// Never read input values, follow frames, click, fill, or return page text.
// Closed result: 0 is clear; 1..18 name only a structural reason to keep holding.
(() => {
  try {
    if (!document.body || !["interactive", "complete"].includes(document.readyState)) return 1;
    const visible = (node) => {
      if (!node || node.closest('[hidden],[inert],[aria-hidden="true"]')) return false;
      const box = node.getBoundingClientRect();
      const style = getComputedStyle(node);
      return box.width > 0 && box.height > 0 && style.display !== "none" &&
        style.visibility !== "hidden" && style.opacity !== "0";
    };
    const searchEntry = (node) => node.matches("input[type='search'],[role='searchbox']") &&
      (node.getAttribute("type") || "").toLowerCase() !== "email" &&
      !node.closest("form:not([role='search']),dialog,[role='dialog'],[aria-modal='true']");
    const active = document.activeElement;
    if (visible(active) && active.matches("input,textarea") && !searchEntry(active)) {
      const hint = ["type", "autocomplete", "name", "id", "aria-label", "placeholder"].map((name) => active.getAttribute(name) || "").join(" ");
      if (/(?:password|one-time-code|totp|otp|authentication.?code|verification.?code|confirmation.?code|email|user.?name|workspace)/i.test(hint)) return 2;
    }
    const nodes = document.querySelectorAll("input,textarea,select,iframe,ul,ol,[role='list'],h1,h2,h3,h4,h5,h6,p,button,a,main,article,li,tr,[role='heading'],[role='alert'],[role='main'],[role='dialog'],[role='row'],[role='listitem'],[role='option'],[role='link'],[role='button'],[role='grid'],[role='status'],[role='progressbar'],[role='textbox'],[contenteditable='true'],[aria-busy='true']");
    if (nodes.length > 4096) return 3;
    const challenge = /^(?:just a moment|checking (?:your browser|if the site connection)|verifying you are human|verify you are human|please verify|confirm you are human|are you (?:a human|a robot)|prove you are human|press (?:&|and) hold|human verification|security (?:check|verification)|complete the security check|pardon our interruption|access denied|unusual traffic|something went wrong|please wait)/i;
    const signIn = /(?:sign[ -]?in|log[ -]?in|zaloguj|two[ -]?factor|verification code|confirmation code|authentication code|one[ -]?time code|choose (?:an? |your )?(?:account|workspace|organization)|select (?:an? |your )?(?:account|workspace|organization)|find your workspace|workspace url|enter your workspace|continue (?:with|to) (?:google|microsoft|apple|slack|sso)|single sign[ -]?on)/i;
    const authStatus = /^(?:(?:please\s+)?(?:check|open)\s+(?:your\s+)?(?:e[ -]?mail|inbox)|(?:enter|type|provide)\s+(?:(?:the|your|a)\s+)?(?:(?:six|6)[ -]digit\s+)?(?:(?:confirmation|verification|authentication|security|one[ -]?time)\s+)?code|(?:approve|confirm|complete)\s+(?:the\s+|your\s+)?(?:sign[ -]?in|log[ -]?in|authentication)|(?:we(?:'ve| have)?\s+)?sent\s+(?:you\s+)?(?:an?\s+|the\s+)?(?:e[ -]?mail|sign[ -]?in link|verification|confirmation)|waiting\s+for\s+(?:approval|verification|authentication))/i;
    const label = (node) => {
      const aria = node.getAttribute("aria-label");
      if (aria) return aria.slice(0, 160).trim();
      const pending = [node.firstChild];
      let text = "", visited = 0;
      while (pending.length && visited++ < 24 && text.length < 160) {
        const next = pending.pop();
        if (!next) continue;
        if (next.nextSibling) pending.push(next.nextSibling);
        if (next.nodeType === 3) text += (next.nodeValue || "").slice(0, 160 - text.length);
        else if (next.nodeType === 1 && !next.matches("input,textarea,select,[contenteditable='true']"))
          pending.push(next.firstChild);
      }
      return text.trim();
    };
    const publicEntry = (node) => {
      if (searchEntry(node)) return true;
      const form = node.closest("form,[role='search']");
      if (form?.getAttribute("role") === "search") return true;
      if (!form) return false;
      if (/subscribe|newsletter|mailing[-_ ]list/i.test((form.getAttribute("action") || "").slice(0, 256))) return true;
      return Array.from(form.querySelectorAll("button,[role='button']")).slice(0, 8)
        .some((button) => /^(?:subscribe|sign up for (?:our |the )?newsletter|join (?:our |the )?mailing list)\b/i.test(label(button)));
    };
    let heading = false, paragraph = false, links = 0, main = false, rows = 0;
    let authHeading = false, authControls = 0, editable = 0, unknownEntry = 0, busy = false;
    const cardGroups = new Map();
    let cards = 0, lists = 0, composers = 0;
    for (const node of nodes) {
      if (!visible(node)) continue;
      const tag = node.tagName.toLowerCase();
      const role = node.getAttribute("role") || "";
      if (node.getAttribute("aria-busy") === "true" || role === "progressbar") busy = true;
      if (["input", "textarea", "select"].includes(tag) || role === "textbox" || node.isContentEditable) {
        const type = (node.getAttribute("type") || "").toLowerCase();
        const hints = ["autocomplete", "name", "id", "aria-label", "placeholder"].map((name) => node.getAttribute(name) || "");
        if (hints.some((hint) => hint.length > 256)) return 4;
        const hint = hints.join(" ");
        if (type === "password" || /(?:password|one-time-code|totp|otp|authentication.?code|verification.?code|confirmation.?code)/i.test(hint)) return 5;
        // Segmented OTP widgets often expose only ordinary one-character
        // text inputs, without an accessible code label or autocomplete hint.
        if (node.getAttribute("maxlength") === "1" && /^(?:text|tel|number|)$/.test(type)) return 6;
        if (node.disabled || node.readOnly) continue;
        editable++;
        if (!publicEntry(node) && type !== "search") {
          unknownEntry++;
          if ((tag === "textarea" || (node.isContentEditable && node.getAttribute("aria-multiline") === "true")) &&
              !node.closest("form,dialog,[role='dialog'],[aria-modal='true']")) composers++;
          if (node.closest("dialog,[role='dialog'],[aria-modal='true']")) return 14;
        }
        if ((/(?:email|user.?name|workspace|organization|tenant)/i.test(hint) || type === "email") && !publicEntry(node)) return 7;
        continue;
      }
      if (tag === "iframe") {
        const title = (node.getAttribute("title") || "").toLowerCase();
        const src = (node.getAttribute("src") || "").toLowerCase();
        const width = node.getBoundingClientRect().width;
        if (/(?:hcaptcha|captcha challenge|recaptcha challenge|security challenge|verify you are human|arkose)/.test(title) ||
          (title === "recaptcha" && width >= 290) ||
          (/(?:captcha-delivery\.com|challenges\.cloudflare\.com|hcaptcha\.com)/.test(src) && width >= 290)) return 8;
        continue;
      }
      if (["main", "article"].includes(tag) || role === "main" || role === "grid") {
        main = true;
        if (signIn.test((node.getAttribute("aria-label") || "").slice(0, 160))) return 9;
        continue;
      }
      // Bounded labels only; structural containers never expand their text.
      const short = label(node);
      const isHeading = /^h[1-6]$/.test(tag) || role === "heading";
      if ((isHeading || role === "alert" || tag === "p") && challenge.test(short)) return 10;
      if ((isHeading || role === "alert" || role === "status" || role === "dialog") && authStatus.test(short)) return 11;
      if (role === "status" && /^(?:loading|please wait|just a moment)\b/i.test(short)) busy = true;
      if ((isHeading || role === "dialog") && /(?:choose|select|find|enter) (?:an? |your )?(?:account|workspace|organization)|workspace url/i.test(short)) return 12;
      if (isHeading || role === "dialog") authHeading ||= signIn.test(short);
      if ((tag === "button" || tag === "a" || role === "button" || role === "link") && signIn.test(short)) authControls++;
      // Legal/help chrome is not evidence that the requested app loaded.
      if (node.closest("nav,footer,aside,[role='navigation'],[role='contentinfo'],[role='complementary']")) continue;
      if ((["ul", "ol"].includes(tag) || role === "list") && short.length >= 24) lists++;
      // Custom app lists often expose each item as a button rather than li/row.
      // Require repeated local siblings; never count global or auth chrome.
      if ((tag === "button" || role === "button") && short.length >= 12 && !signIn.test(short) &&
          !/^(?:continue|sign|log|authenticate|connect)\b/i.test(short) &&
          !node.closest("form,dialog,[role='dialog'],[role='alert']") &&
          !node.parentElement?.closest("button,[role='button']")) {
        let parent = node.parentElement;
        for (let depth = 0; parent && depth < 6; depth++, parent = parent.parentElement) {
          if (parent === document.body || parent === document.documentElement ||
              parent.matches("nav,footer,aside,form,dialog,[role='navigation'],[role='dialog']")) break;
          if (!cardGroups.has(parent) && cardGroups.size >= 512) return 3;
          const count = (cardGroups.get(parent) || 0) + 1;
          cardGroups.set(parent, count);
          cards = Math.max(cards, count);
        }
      }
      heading ||= isHeading && short.length > 2 && !signIn.test(short);
      paragraph ||= tag === "p" && short.length >= 24;
      links += Number(((tag === "a" && !!node.getAttribute("href")) || role === "link") && short.length > 1 && !!node.closest("main,article,[role='main'],[role='grid']"));
      rows += Number((["li", "tr"].includes(tag) || ["row", "listitem", "option"].includes(role)) && short.length >= 8 && !!node.closest("main,[role='main'],[role='grid'],[role='list']"));
    }
    // Authentication forms may arrive after their heading, and email/MFA
    // waiting screens may have no editable controls at all.
    if (authHeading) return 13;
    // A heading and explanatory paragraph also describe most code-entry
    // screens. Unclassified editable controls require actual app structure.
    // A message composer is not an unknown identity field when a populated
    // conversation and repeated local app controls are already visible.
    const conversation = heading && lists > 0 && cards >= 3;
    if (unknownEntry > 0 && !(main && rows >= 3) &&
        !(conversation && composers > 0 && unknownEntry === composers)) return 14;
    if (busy && !(main && (links >= 3 || rows >= 3)) && !conversation) return 15;
    if (authControls > 0 && editable > 0 && !(heading && paragraph) && rows < 3) return 16;
    // A settled URL and the absence of a field are insufficient: require
    // visible page content, so an empty SPA shell/spinner cannot resume it.
    return ((heading && (paragraph || cards >= 3)) || (main && (links >= 3 || rows >= 3))) ? 0 : 17;
  } catch {
    return 18;
  }
})();
