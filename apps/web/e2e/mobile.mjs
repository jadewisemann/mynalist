import { chromium, devices } from 'playwright-core';
const exe = `${process.env.HOME}/.cache/ms-playwright/chromium-1228/chrome-linux/chrome`;
const browser = await chromium.launch({ executablePath: exe });
const ctx = await browser.newContext({ ...devices['Pixel 7'] });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
await page.goto(process.env.URL ?? 'http://localhost:5174/');
await page.waitForSelector('.o-editor');
await page.waitForTimeout(400);
await page.screenshot({ path: '/tmp/m1.png' });
// tap an item, type, use toolbar indent
await page.tap('.o-content >> text=Press Ctrl+/');
await page.keyboard.press('End');
await page.keyboard.press('Enter');
await page.keyboard.type('typed on phone');
await page.tap('.mobile-bar button:nth-child(2)'); // indent
await page.waitForTimeout(200);
const depth = await page.evaluate(() => {
  const n = [...document.querySelectorAll('.o-node')].find((n) => n.querySelector(':scope > .o-row .o-content').textContent === 'typed on phone');
  return n?.parentElement.classList.contains('o-kids');
});
console.log(depth ? 'ok   toolbar indent' : 'FAIL toolbar indent');
await page.screenshot({ path: '/tmp/m2.png' });
await page.tap('button[title="Toggle sidebar"]');
await page.waitForTimeout(300);
await page.screenshot({ path: '/tmp/m3.png' });
console.log('errors', errors);
await browser.close();
