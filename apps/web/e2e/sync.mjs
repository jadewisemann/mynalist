// Needs a dev server started with the same OUTLINER_PASSWORD in the environment.
import { chromium } from 'playwright-core';
const exe = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const URL = process.env.URL ?? process.env.URL ?? 'http://localhost:5174/';
const browser = await chromium.launch({ executablePath: exe });
const errors = [];

async function client(name) {
  const ctx = await browser.newContext({ viewport: { width: 1100, height: 760 } });
  const page = await ctx.newPage();
  page.on('pageerror', (e) => errors.push(`${name}: ${e}`));
  await page.goto(URL);
  await page.waitForSelector('.o-editor');
  // connect via settings
  await page.click('button[title=Settings]');
  await page.fill('label.field:has-text("Sync server URL") input', URL.replace(/\/$/, ''));
  await page.fill('input[type=password]', process.env.OUTLINER_PASSWORD ?? '');
  await page.click('text=Connect');
  await page.waitForSelector('text=Connected.', { timeout: 10000 });
  await page.keyboard.press('Escape');
  return page;
}

const outline = (page) => page.evaluate(() => {
  const walk = (el, d) => [...el.children].map((n) => '  '.repeat(d) + n.querySelector(':scope > .o-row .o-content').textContent + '\n' + walk(n.querySelector(':scope > .o-kids'), d + 1)).join('');
  return walk(document.querySelector('.o-items'), 0);
});

const A = await client('A');
await A.waitForTimeout(1500);
// A creates a doc
await A.click('.pane-head button');
await A.click('text=New document');
await A.keyboard.type('Shared doc');
await A.keyboard.press('Enter');
await A.waitForTimeout(300);
await A.click('.o-editor', { position: { x: 200, y: 140 } });
await A.keyboard.type('alpha');
await A.keyboard.press('Enter');
await A.keyboard.type('beta');
await A.keyboard.press('Tab');
await A.waitForTimeout(2500);

const B = await client('B');
await B.waitForTimeout(2500);
const files = await B.$$eval('.file-row .name', (els) => els.map((e) => e.textContent));
console.log('B files:', files);
await B.click('.file-row:has-text("Shared doc")');
await B.waitForTimeout(500);
console.log('B sees:\n' + await outline(B));

// B edits, A should receive via push
await B.click('.o-items .o-node .o-content >> text=alpha');
await B.keyboard.press('End');
await B.keyboard.type(' (edited by B)');
await B.waitForTimeout(3000);
console.log('A sees after B edit:\n' + await outline(A));

// concurrent: both add an item at the same time
await A.click('.o-items .o-node .o-kids .o-content >> text=beta');
await A.keyboard.press('End');
await A.keyboard.press('Enter');
await A.keyboard.type('from A');
await B.click('.o-items .o-node .o-kids .o-content >> text=beta');
await B.keyboard.press('End');
await B.keyboard.press('Enter');
await B.keyboard.type('from B');
await A.keyboard.press('Escape');
await B.keyboard.press('Escape');
await A.waitForTimeout(4000);
const oa = await outline(A), ob = await outline(B);
console.log('A:\n' + oa + 'B:\n' + ob + 'converged:', oa === ob);
console.log('sync labels:', await A.textContent('.sync-status'), '/', await B.textContent('.sync-status'));
console.log('errors:', errors);
await browser.close();
