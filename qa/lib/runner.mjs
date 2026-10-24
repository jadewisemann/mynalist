// Drives one scenario on one app: fresh doc → paste fixture → setup → steps → state (+ screenshot).

import { chromium } from 'playwright-core';
import { APPS, EXE } from './apps.mjs';

export const KEY_DELAY = 60; // Dynalist handles some keys asynchronously; fast sequences reorder

let browser;
export async function getBrowser() {
  browser ??= await chromium.launch({ executablePath: EXE });
  return browser;
}
export async function closeBrowser() {
  await browser?.close();
  browser = undefined;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export async function press(page, key) {
  await page.keyboard.press(key);
  await sleep(KEY_DELAY);
}

export async function dispatchClipboard(page, type, text) {
  return page.evaluate(
    ([type, text]) => {
      const dt = new DataTransfer();
      if (text != null && typeof text === 'object') for (const [k, v] of Object.entries(text)) v && dt.setData(k, v);
      else if (text != null) dt.setData('text/plain', text);
      const target = document.activeElement ?? document.body;
      target.dispatchEvent(new ClipboardEvent(type, { clipboardData: dt, bubbles: true, cancelable: true }));
      return { text: dt.getData('text/plain'), html: dt.getData('text/html') };
    },
    [type, text],
  );
}

/** Put the caret in item i (-1 = title): at = 'start' | 'end' | number; f = 'c' | 'n'. */
export async function focusAt(app, page, i, at = 'end', f = 'c') {
  const loc = await app.item(page, i);
  await loc.click({ position: { x: 3, y: 6 } });
  await sleep(KEY_DELAY);
  if (f === 'n') {
    await press(page, 'End');
    await press(page, 'Shift+Enter');
  }
  if (at === 'end') await press(page, 'End');
  else {
    await press(page, 'Home');
    for (let k = 0; k < (at === 'start' ? 0 : at); k++) await press(page, 'ArrowRight');
  }
}

/**
 * Step forms: 'Key+Combo' | { type } | { paste } | { pasteClip } | { copy } | { cut } | { at: [i, at?, f?] } |
 * { wait } | { click: 'bullet' | 'content', i }
 */
export async function step(app, page, s, out) {
  if (typeof s === 'string') return press(page, s);
  if (s.type != null) {
    await page.keyboard.type(s.type, { delay: 25 });
    return sleep(KEY_DELAY);
  }
  if (s.paste != null || s.pasteClip) {
    await dispatchClipboard(page, 'paste', s.pasteClip ? { 'text/plain': out.clip ?? '', 'text/html': out.clipHtml ?? '' } : s.paste);
    return sleep(150);
  }
  if (s.copy || s.cut) {
    const r = await dispatchClipboard(page, s.copy ? 'copy' : 'cut', null);
    out.clip = r.text;
    out.clipHtml = r.html;
    return sleep(100);
  }
  if (s.at) return focusAt(app, page, ...s.at);
  if (s.wait) return sleep(s.wait);
  throw new Error(`bad step ${JSON.stringify(s)}`);
}

/** Setup ops applied after the fixture paste, last item first so indices stay valid. */
const SETUP = {
  note: async (app, page, i, text) => {
    await focusAt(app, page, i, 'end');
    await press(page, 'Shift+Enter');
    await page.keyboard.type(text, { delay: 20 });
    await press(page, 'Shift+Enter');
  },
  check: async (app, page, i) => (await focusAt(app, page, i), press(page, 'Control+Enter')),
  collapse: async (app, page, i) => (await focusAt(app, page, i), press(page, 'Control+Period')),
  checkbox: async (app, page, i) => (await focusAt(app, page, i), press(page, 'Control+Shift+C')),
  heading: async (app, page, i, n = 1) => {
    await focusAt(app, page, i);
    for (let k = 0; k < n; k++) await press(page, 'Control+Shift+H');
  },
  numbered: async (app, page, i) => (await focusAt(app, page, i), press(page, 'Control+Shift+X')),
};

export async function runScenario(appName, sc, { shot } = {}) {
  const app = APPS[appName];
  const b = await getBrowser();
  const ctx = await b.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await ctx.newPage();
  page.setDefaultTimeout(8000);
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e.message ?? e)));
  const out = { errors };
  try {
    await app.fresh(page);
    if (sc.fx) await step(app, page, { paste: sc.fx }, out);
    const setup = [...(sc.setup ?? [])].sort((a, b) => b[1] - a[1]);
    for (const [op, i, ...args] of setup) await SETUP[op](app, page, i, ...args);
    if (sc.setup?.length || sc.fx) {
      await press(page, 'Escape');
      await press(page, 'Escape');
    }
    out.pre = await app.state(page);
    out.trace = [];
    for (const s of sc.steps) {
      await step(app, page, s, out);
      if (sc.trace) out.trace.push(await app.state(page));
    }
    await sleep(150);
    out.post = await app.state(page);
    if (sc.render) {
      await page.evaluate(() => document.activeElement?.blur()); // every item shows its rendered form
      await sleep(250);
      out.post.render = await app.render(page);
      out.post.focus = null; // rendering is what's compared here
      out.post.selected = [];
    }
    if (shot) out.shot = await page.screenshot({ type: 'jpeg', quality: 60, clip: await app.clip(page) });
  } catch (e) {
    out.fatal = String(e.message ?? e).split('\n')[0];
    try {
      out.post ??= await app.state(page);
      if (shot) out.shot = await page.screenshot({ type: 'jpeg', quality: 60 });
    } catch {}
  } finally {
    await ctx.close();
  }
  return out;
}
