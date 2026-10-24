// Uploads (paste / drop), share links and the calendar feed against a real server serving the web build.
//   apps/web/e2e/snapshot.sh && node apps/web/e2e/server-extras.mjs
import { chromium } from 'playwright-core';
import { spawn } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const root = new URL('../../..', import.meta.url).pathname;
const dir = mkdtempSync(join(tmpdir(), 'outliner-e2e-'));
const PORT = 5199;
const ORIGIN = `http://127.0.0.1:${PORT}`;
const password = randomBytes(12).toString('base64url');
const srv = spawn(process.execPath, ['apps/server/src/main.ts'], {
  cwd: root,
  env: { ...process.env, OUTLINER_TOKEN: randomBytes(16).toString('hex'), OUTLINER_PASSWORD: password, OUTLINER_DB: join(dir, 'e2e.db'),
    OUTLINER_STATIC: '/tmp/websnap', PORT: String(PORT), OUTLINER_HOST: '127.0.0.1' },
  stdio: 'inherit',
});
for (let i = 0; i < 60; i++) {
  if (await fetch(`${ORIGIN}/api/health`).then((r) => r.ok, () => false)) break;
  await new Promise((r) => setTimeout(r, 250));
}

const LOCAL = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || (existsSync(LOCAL) ? LOCAL : undefined) });
const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
let fails = 0;
const check = (label, ok, info = '') => {
  if (!ok) fails++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${label}${ok ? '' : ` ${info}`}`);
};
const K = (k) => page.keyboard.press(k);

try {
  await page.goto(ORIGIN + '/');
  await page.waitForSelector('.o-editor');
  await page.click('button[title=Settings]');
  await page.click('.set-tabs .tab:has-text("Sync & data")');
  await page.fill('label.field:has-text("Sync server URL") input', ORIGIN);
  await page.fill('input[type=password]', password);
  await page.click('text=Connect');
  await page.waitForSelector('text=Connected.', { timeout: 10000 });
  await K('Escape');

  await page.click('.pane-head button[title^="New"]');
  await page.click('text=New document');
  await page.keyboard.type('Trip');
  await K('Enter');
  await page.waitForTimeout(200);
  await page.click('.o-title .o-content');
  await K('End');
  await K('Enter');
  await page.keyboard.type('Flight !(2026-11-02 08:15) ');

  // paste an image file
  await page.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add(new File([new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10])], 'shot (1).png', { type: 'image/png' }));
    document.activeElement.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  });
  await page.waitForFunction(() => document.activeElement?.textContent?.includes('/files/'), null, { timeout: 5000 }).catch(() => {});
  const src = await page.evaluate(() => document.activeElement?.textContent ?? '');
  check('paste uploads and links the image', /^Flight !\(2026-11-02 08:15\) !\[shot \\?\(1\\?\)\.png\]\(http:\/\/127\.0\.0\.1:5199\/files\/[\w-]+\/shot%20%281%29\.png\)$/.test(src), src);
  const fileUrl = /\((http[^)]+)\)$/.exec(src)?.[1];
  const got = fileUrl && (await fetch(fileUrl));
  check('uploaded file is served', got?.status === 200 && got.headers.get('content-type') === 'image/png');

  // drop a text file onto the item: a new item after it
  await K('Escape');
  await page.evaluate(() => {
    const row = [...document.querySelectorAll('.o-items .o-node')].at(-1);
    const r = row.getBoundingClientRect();
    const dt = new DataTransfer();
    dt.items.add(new File(['itinerary'], 'plan.txt', { type: 'text/plain' }));
    for (const type of ['dragover', 'drop']) {
      row.dispatchEvent(new DragEvent(type, { dataTransfer: dt, clientX: r.x + 60, clientY: r.y + r.height / 2, bubbles: true, cancelable: true }));
    }
  });
  await page.waitForFunction(() => [...document.querySelectorAll('.o-items .o-content')].some((e) => e.textContent.includes('plan.txt')), null, { timeout: 5000 }).catch(() => {});
  const items = await page.$$eval('.o-items > .o-node > .o-row .o-content', (els) => els.map((e) => e.textContent));
  check('drop creates an item linking the file', items.length === 2 && /^\[plan\.txt\]\(http:\/\/127\.0\.0\.1:5199\/files\/[\w-]+\/plan\.txt\)$/.test(items[1]), JSON.stringify(items));
  await page.waitForTimeout(1500); // let it sync

  // share the document
  await page.hover('.file-row:has-text("Trip")');
  await page.click('.file-row:has-text("Trip") .more');
  await page.click('.menu .mi:has-text("Share")');
  await page.click('.modal button:has-text("Create link")');
  await page.waitForSelector('.share-row input');
  const shareUrl = await page.inputValue('.share-row input');
  const pub = await browser.newPage();
  await pub.goto(shareUrl);
  check('share page shows the doc', (await pub.textContent('h1')) === 'Trip' && (await pub.textContent('main')).includes('Flight'));
  check('share page shows the image', (await pub.$$('main img')).length === 1);
  await page.click('.modal .share-row button:has-text("Revoke")');
  await page.waitForTimeout(300);
  check('revoked link is gone', (await fetch(shareUrl)).status === 404);
  await K('Escape');

  // settings: calendar feed, shared links, uploaded files
  await page.click('button[title=Settings]');
  await page.click('.set-tabs .tab:has-text("Sync & data")');
  await page.waitForSelector('section:has-text("Calendar feed") input');
  const cal = await page.inputValue('section:has-text("Calendar feed") input');
  const ics = await (await fetch(cal)).text();
  check('calendar feed has the dated item', ics.includes('DTSTART:20261102T081500') && ics.includes('SUMMARY:Flight shot (1).png'), ics.slice(0, 400));
  await page.waitForSelector('section:has-text("Uploaded files") .share-row');
  check('upload manager lists both files', (await page.$$('section:has-text("Uploaded files") .share-row')).length === 2);
  check('no page errors', errors.length === 0, errors.join('\n'));
} finally {
  await browser.close();
  srv.kill();
  rmSync(dir, { recursive: true, force: true });
}
console.log(fails ? `${fails} FAILED` : 'all ok');
process.exit(fails ? 1 : 0);
