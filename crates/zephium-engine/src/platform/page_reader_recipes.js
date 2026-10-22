// Fixed Ask panel recipes for the private world: the selected text, a quote
// scrolled to and marked, and the page's outline for a style turn. Reached
// only through closed `Q1:` invocations; every page string is data.
(() => {
  "use strict";
  const NAME = "__zephiumPageAssistV1";
  if (Object.hasOwn(globalThis, NAME)) return;
  const doc = document;
  const apply = Reflect.apply, parse = JSON.parse, stringify = JSON.stringify;
  const style = getComputedStyle, now = () => performance.now();
  const rectOf = Element.prototype.getBoundingClientRect;
  const query = Document.prototype.querySelectorAll, queryIn = Element.prototype.querySelectorAll;
  const MAX_SELECTION = 4096, MAX_QUOTE = 2048, MAX_HAY = 2000000, MAX_TEXT_NODES = 100000;
  const MAX_NODES = 48, MAX_SCAN = 3000, MAX_OUTLINE_BYTES = 24 * 1024;
  const SKIP = new Set(["SCRIPT", "STYLE", "NOSCRIPT", "TEMPLATE", "TEXTAREA", "INPUT", "SELECT", "OPTION", "SVG", "CANVAS"]);
  const TEXT_TYPES = new Set(["", "text", "search", "url", "email"]);
  const SECRET_HINT = /(?:pass|pwd|otp|one-?time|pin|cvv|cvc|csc|card|ccnum|cc-|security.?code|verification)/i;

  const lower = (value) => String(value || "").toLowerCase();
  // Exactly JavaScript's \s, without a regular expression per character.
  const space = (code) => code === 32 || (code >= 9 && code <= 13) || code === 0xa0 || code === 0x1680 ||
    (code >= 0x2000 && code <= 0x200a) || code === 0x2028 || code === 0x2029 || code === 0x202f ||
    code === 0x205f || code === 0x3000 || code === 0xfeff;
  const attribute = (element, name) => {
    const value = element.getAttribute(name);
    return typeof value === "string" ? value.slice(0, 256) : "";
  };

  function protectedField(element) {
    if (lower(element.type) === "password") return true;
    const auto = lower(attribute(element, "autocomplete"));
    if (auto.includes("cc-") || auto.includes("one-time-code") || auto.includes("password")) return true;
    const hints = [attribute(element, "name"), element.id || "", attribute(element, "aria-label"), attribute(element, "placeholder")].join(" ");
    return SECRET_HINT.test(hints) || attribute(element, "maxlength") === "1";
  }

  function selection() {
    let text = "";
    const active = doc.activeElement;
    const tag = active && active.tagName;
    if (tag === "TEXTAREA" || (tag === "INPUT" && TEXT_TYPES.has(lower(active.type)))) {
      if (protectedField(active)) return "N";
      const start = active.selectionStart, end = active.selectionEnd;
      if (typeof start === "number" && typeof end === "number" && end > start) {
        text = String(active.value).slice(start, Math.min(end, start + MAX_SELECTION));
      }
    }
    if (!text) {
      const current = getSelection();
      if (current && !current.isCollapsed && current.rangeCount > 0) text = String(current).slice(0, MAX_SELECTION * 2);
    }
    text = text.replace(/\s+/gu, " ").trim().slice(0, MAX_SELECTION);
    return text ? "T:" + text : "N";
  }

  // Page text as one string with runs of white space collapsed, and where
  // each text node starts in it, so a match maps back to a DOM range.
  function textIndex(root) {
    const segments = [], parts = [];
    let length = 0, gap = true, nodes = 0;
    const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const parent = node.parentElement;
        return parent && !SKIP.has(parent.tagName) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
      }
    });
    for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
      if (++nodes > MAX_TEXT_NODES || length > MAX_HAY) break;
      const data = node.data;
      let out = "";
      const startGap = gap;
      for (let index = 0; index < data.length; index += 1) {
        if (space(data.charCodeAt(index))) {
          if (!gap) { out += " "; gap = true; }
        } else {
          out += data[index];
          gap = false;
        }
      }
      if (out.length === 0) continue;
      segments.push({ start: length, node, gap: startGap });
      parts.push(out);
      length += out.length;
    }
    return { hay: parts.join(""), segments };
  }

  function locate(index, position, end) {
    const segments = index.segments;
    let low = 0, high = segments.length - 1;
    while (low < high) {
      const middle = (low + high + 1) >> 1;
      if (segments[middle].start <= position - (end ? 1 : 0)) low = middle; else high = middle - 1;
    }
    const segment = segments[low];
    if (!segment) return null;
    const data = segment.node.data;
    let collapsed = segment.start, gap = segment.gap;
    for (let raw = 0; raw < data.length; raw += 1) {
      const white = space(data.charCodeAt(raw));
      if (white && gap) continue;
      if (!end && collapsed === position) return [segment.node, raw];
      collapsed += 1;
      gap = white;
      if (end && collapsed === position) return [segment.node, raw + 1];
    }
    return end ? [segment.node, data.length] : null;
  }

  function find(index, quote) {
    const needle = quote.replace(/\s+/gu, " ").trim();
    if (needle.length < 2) return null;
    let at = index.hay.indexOf(needle), size = needle.length;
    if (at < 0) at = lower(index.hay).indexOf(lower(needle));
    if (at < 0 && needle.length > 120) {
      const head = needle.slice(0, 120);
      at = index.hay.indexOf(head);
      size = head.length;
    }
    if (at < 0) return null;
    const start = locate(index, at, false), end = locate(index, at + size, true);
    if (!start || !end) return null;
    const range = doc.createRange();
    range.setStart(start[0], start[1]);
    range.setEnd(end[0], end[1]);
    return range;
  }

  function opaque(color) {
    const match = /^rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)$/u.exec(color || "");
    if (!match) return null;
    if (match[4] !== undefined && Number(match[4]) < 0.98) return null;
    return [Number(match[1]), Number(match[2]), Number(match[3])];
  }

  function solid(color) {
    const match = /^rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)$/u.exec(color || "");
    return match ? [Number(match[1]), Number(match[2]), Number(match[3])] : null;
  }

  function luminance(rgb) {
    const channel = (value) => {
      const c = value / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);
  }

  function backdrop(element) {
    for (let current = element, depth = 0; current && depth < 64; current = current.parentElement, depth += 1) {
      const color = opaque(style(current).backgroundColor);
      if (color) return color;
    }
    return null;
  }

  // Neutral and per document, so a page cannot recognise the panel by it.
  const highlightName = "h" + Math.random().toString(36).slice(2, 10);
  let mark = null;
  function unmark() {
    if (mark === null) return;
    const current = mark;
    mark = null;
    current.done = true;
    try { current.highlight.clear(); } catch (_) {}
    try { CSS.highlights.delete(current.name); } catch (_) {}
    try { doc.adoptedStyleSheets = doc.adoptedStyleSheets.filter((sheet) => sheet !== current.sheet); } catch (_) {}
  }

  function scrollTo(range, reduced) {
    const host = range.startContainer.parentElement;
    if (!host) return;
    const behavior = reduced ? "auto" : "smooth";
    const box = apply(rectOf, host, []);
    if (box.height > 0 && box.height <= innerHeight * 0.8) {
      host.scrollIntoView({ block: "center", inline: "nearest", behavior });
      return;
    }
    host.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "auto" });
    const target = range.getBoundingClientRect();
    scrollBy({ top: target.top - innerHeight * 0.3, behavior });
  }

  // A soft highlight under the quote's own text: no element is added, so
  // nothing reflows. It eases in, holds, then fades and is gone.
  function markQuote(encoded) {
    let quote;
    try { quote = parse(encoded); } catch (_) { return "E"; }
    if (typeof quote !== "string" || quote.length > MAX_QUOTE) return "E";
    const root = doc.body || doc.documentElement;
    if (!root) return "0";
    const range = find(textIndex(root), quote);
    if (range === null) return "0";
    unmark();
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    scrollTo(range, reduced);
    if (typeof Highlight !== "function" || !globalThis.CSS || !CSS.highlights || typeof CSSStyleSheet !== "function") return "2";
    const ground = backdrop(range.startContainer.parentElement) || backdrop(root);
    const dark = ground ? luminance(ground) < 0.35 : matchMedia("(prefers-color-scheme: dark)").matches;
    const tint = dark ? "250 204 21" : "255 214 102";
    const peak = dark ? 0.34 : 0.62;
    const name = highlightName;
    // A hidden page draws no frames: show the mark at once and let it fade
    // when the page is seen again.
    const still = reduced || doc.visibilityState === "hidden";
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(`::highlight(${name}){background-color:rgb(${tint} / ${still ? peak : 0});}`);
    const rule = sheet.cssRules[0];
    try {
      CSS.highlights.set(name, new Highlight(range));
      doc.adoptedStyleSheets = [...doc.adoptedStyleSheets, sheet];
    } catch (_) {
      try { CSS.highlights.delete(name); } catch (_) {}
      return "2";
    }
    const current = { name, sheet, highlight: CSS.highlights.get(name), done: false };
    mark = current;
    const started = now(), rise = still ? 0 : 220, hold = 2600, fall = 1400;
    const frame = () => {
      if (current.done) return;
      const elapsed = now() - started;
      let alpha;
      if (elapsed < rise) alpha = peak * (elapsed / rise);
      else if (elapsed < rise + hold) alpha = peak;
      else if (elapsed < rise + hold + fall) {
        const t = (elapsed - rise - hold) / fall;
        alpha = peak * (1 - t) * (1 - t);
      } else {
        if (mark === current) unmark();
        return;
      }
      rule.style.backgroundColor = `rgb(${tint} / ${alpha.toFixed(3)})`;
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
    // Frames stop while the page is hidden; the mark still goes on time.
    setTimeout(() => { if (mark === current) unmark(); }, rise + hold + fall + 120);
    return "1";
  }

  const IDENT = /^[A-Za-z][A-Za-z0-9_-]{0,31}$/u;
  const GENERATED = /^(?:css|sc|jsx|emotion|svelte|astro|tw|ng|chakra|mantine)-/iu;
  function stable(token) {
    if (!IDENT.test(token) || GENERATED.test(token)) return false;
    if ((token.match(/\d/gu) || []).length >= 3) return false;
    for (const part of token.split(/[-_]+/u)) {
      if (part.length >= 5 && /\d/u.test(part) && /[A-Za-z]/u.test(part)) return false;
      if (part.length <= 8) {
        let turns = 0;
        for (let index = 1; index < part.length; index += 1) {
          const upper = part[index] !== part[index].toLowerCase();
          const previous = part[index - 1] !== part[index - 1].toLowerCase();
          if (upper !== previous) turns += 1;
        }
        if (turns >= 4) return false;
      }
      if (part.length >= 6 && /^[a-f0-9]+$/iu.test(part) && /\d/u.test(part)) return false;
    }
    return true;
  }

  function selectorOf(element) {
    let selector = lower(element.tagName);
    if (!/^[a-z][a-z0-9-]{0,31}$/u.test(selector)) return null;
    const id = element.id;
    if (typeof id === "string" && id && stable(id)) selector += "#" + id;
    let classes = 0;
    for (const token of element.classList) {
      if (classes >= 3) break;
      if (stable(token)) { selector += "." + token; classes += 1; }
    }
    return selector;
  }

  function visible(element) {
    const box = apply(rectOf, element, []);
    return box.width >= 2 && box.height >= 2;
  }

  function outline() {
    const root = doc.body;
    if (!root) return "E";
    const chosen = [], seen = new Set();
    const add = (element, kind) => {
      if (chosen.length >= MAX_NODES || !visible(element)) return;
      const selector = selectorOf(element);
      if (!selector || seen.has(selector)) return;
      seen.add(selector);
      chosen.push({ element, kind, selector });
    };
    const each = (selector, kind, limit) => {
      const found = apply(query, doc, [selector]);
      for (let index = 0, taken = 0; index < found.length && taken < limit; index += 1) {
        const before = chosen.length;
        add(found[index], kind);
        if (chosen.length > before) taken += 1;
      }
    };
    each("header, [role=banner]", "header", 2);
    each("nav, [role=navigation]", "navigation", 3);
    each("main, [role=main]", "main", 1);
    each("article", "article", 2);
    each("aside, [role=complementary]", "aside", 2);
    each("footer, [role=contentinfo]", "footer", 2);
    each("h1", "heading", 1);
    each("h2", "heading", 1);
    each("h3", "heading", 1);

    const counts = new Map();
    const scope = apply(query, doc, ["main, [role=main], article"])[0] || root;
    const walker = doc.createTreeWalker(scope, NodeFilter.SHOW_ELEMENT);
    for (let node = walker.nextNode(), scanned = 0; node !== null && scanned < MAX_SCAN; node = walker.nextNode(), scanned += 1) {
      if (node.classList.length === 0 || SKIP.has(node.tagName)) continue;
      const selector = selectorOf(node);
      if (!selector || !selector.includes(".")) continue;
      const entry = counts.get(selector);
      if (entry) entry.count += 1; else counts.set(selector, { count: 1, element: node });
    }
    const repeated = [...counts.values()]
      .filter((entry) => entry.count >= 3)
      .map((entry) => ({ entry, box: apply(rectOf, entry.element, []) }))
      .filter(({ box }) => box.width >= 120 && box.height >= 40)
      .sort((a, b) => b.entry.count * b.box.width * b.box.height - a.entry.count * a.box.width * a.box.height)
      .slice(0, 8);
    for (const { entry } of repeated) add(entry.element, "content");
    each("button, [role=button], input[type=submit]", "control", 2);
    each("input[type=search], input[type=text], input:not([type])", "control", 1);
    each("a[href]", "other", 1);

    const nodes = [];
    for (const { element, kind, selector } of chosen) {
      const computed = style(element);
      const size = Math.round(parseFloat(computed.fontSize));
      let count = 0;
      try { count = Math.min(65535, apply(query, doc, [selector]).length); } catch (_) { continue; }
      nodes.push({
        s: selector,
        k: kind,
        c: count,
        b: opaque(computed.backgroundColor),
        f: solid(computed.color),
        z: size >= 1 && size <= 200 ? size : null
      });
    }

    const body = style(root);
    const background = opaque(body.backgroundColor) || opaque(style(doc.documentElement).backgroundColor);
    const text = solid(body.color);
    const content = apply(query, doc, ["main, [role=main], article"])[0] || root;
    let link = null;
    const anchors = apply(queryIn, content, ["a[href]"]);
    for (let index = 0; index < anchors.length && index < 64; index += 1) {
      if (visible(anchors[index])) { link = solid(style(anchors[index]).color); break; }
    }
    const tally = new Map();
    const actions = apply(query, doc, ["button, [role=button], input[type=submit], a[href]"]);
    for (let index = 0; index < actions.length && index < 200; index += 1) {
      const color = opaque(style(actions[index]).backgroundColor);
      if (!color) continue;
      const high = Math.max(...color), low = Math.min(...color);
      if (high === 0 || (high - low) / high < 0.25) continue;
      const key = color.join(",");
      const entry = tally.get(key);
      if (entry) entry.count += 1; else tally.set(key, { color, count: 1 });
    }
    let accent = null, best = 0;
    for (const entry of tally.values()) if (entry.count > best) { best = entry.count; accent = entry.color; }

    const fonts = [];
    for (const element of [root, apply(query, doc, ["h1, h2"])[0], content]) {
      if (!element) continue;
      for (const raw of String(style(element).fontFamily).split(",")) {
        const family = raw.trim().replace(/^["']|["']$/gu, "");
        if (fonts.length < 3 && /^[A-Za-z0-9 _-]{1,48}$/u.test(family) && !fonts.includes(family)) fonts.push(family);
      }
    }
    const dark = background ? luminance(background) < 0.35 : text ? luminance(text) > 0.6 : false;
    let wire = stringify({ n: nodes, bg: background, tx: text, ln: link, ac: accent, ft: fonts, dk: dark });
    while (wire.length > MAX_OUTLINE_BYTES && nodes.length > 0) {
      nodes.pop();
      wire = stringify({ n: nodes, bg: background, tx: text, ln: link, ac: accent, ft: fonts, dk: dark });
    }
    return wire;
  }

  // While steps hold the page: did the person press, type or scroll? Our own
  // steps' events are not trusted, so only the person's count.
  let touched = false, listening = false;
  const touch = (event) => { if (event.isTrusted) touched = true; };
  function wasTouched() {
    if (!listening) {
      listening = true;
      for (const type of ["pointerdown", "keydown", "wheel", "touchstart"]) {
        addEventListener(type, touch, { capture: true, passive: true });
      }
    }
    const answer = touched ? "1" : "0";
    touched = false;
    return answer;
  }

  function recipe(encoded) {
    if (encoded === "Q1:touched") return wasTouched();
    if (encoded === "Q1:selection") return selection();
    if (encoded === "Q1:outline") return outline();
    if (typeof encoded === "string" && encoded.startsWith("Q1:mark:")) return markQuote(encoded.slice(8));
    return "E";
  }

  Object.defineProperty(globalThis, NAME, {
    value: Object.freeze({ recipe: Object.freeze(recipe) }),
    writable: false,
    configurable: false,
    enumerable: false
  });
})();
