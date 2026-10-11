// Screenshot the rsm web build in a given UI language.
//   node screenshot.mjs <url> <locale: ar|en> <out.png>
// Needs playwright-core (npm i playwright-core in any scratch folder) and Google Chrome.
import { chromium } from 'playwright-core';

const [url, locale, out] = process.argv.slice(2);
const browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, locale });
const errors = [];
page.on('pageerror', e => errors.push(e.message));
await page.goto(url);
// The wasm bundle is large; give it time to load and draw the start screen.
await page.waitForTimeout(15000);
await page.screenshot({ path: out });
console.log(errors.length ? `page errors: ${errors.join(' | ')}` : 'no page errors');
await browser.close();
