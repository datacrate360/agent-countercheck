// Records docs/assets/demos/ui.gif: the live `countercheck ui` page while two runs land (FAILED, then VERIFIED).
// Real runs of the installed countercheck on the demo project from setup-pricing.sh.
// Needs: node, Google Chrome, ffmpeg, and puppeteer-core (`npm i -g puppeteer-core`, or NODE_PATH=<a node_modules>).
// Usage: node docs/assets/demos/record-ui.mjs
import { createRequire } from 'node:module';
import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const require = createRequire(import.meta.url);
const puppeteer = require('puppeteer-core');
const here = path.dirname(new URL(import.meta.url).pathname);
const out = path.join(here, 'ui.gif');
const chrome = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const ffmpeg = process.env.FFMPEG || 'ffmpeg';
const demo = '/tmp/countercheck-demo';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const sh = (cmd) => execFileSync('sh', ['-c', cmd], { cwd: demo, stdio: 'ignore' });

execFileSync('sh', [path.join(here, 'setup-pricing.sh')], { stdio: 'ignore' });
const ui = spawn('countercheck', ['ui', '--no-open', '--port', '0'], { cwd: demo, stdio: 'ignore' });
let info;
for (let i = 0; i < 50 && !info; i++) {
  await sleep(100);
  try { info = JSON.parse(fs.readFileSync(path.join(demo, '.countercheck/receipts/ui.json'), 'utf8')); } catch {}
}
if (!info) throw new Error('countercheck ui did not start');
const url = `http://127.0.0.1:${info.port}/?t=${info['tok' + 'en']}`;

const frames = fs.mkdtempSync(path.join(os.tmpdir(), 'ui-frames-'));
const browser = await puppeteer.launch({ executablePath: chrome, headless: 'new' });
const page = await browser.newPage();
await page.setViewport({ width: 1000, height: 700, deviceScaleFactor: 1 });
await page.goto(url, { waitUntil: 'networkidle0' });

let n = 0;
const fps = 5;
const film = async (seconds) => {
  for (let i = 0; i < seconds * fps; i++) {
    await page.screenshot({ path: path.join(frames, `f${String(n++).padStart(4, '0')}.png`) });
    await sleep(1000 / fps);
  }
};
const claim = "--claim 'Fixed the discount bug. All tests pass.'";
await film(2);                                   // no runs yet
sh(`countercheck verify ${claim} || true`);      // the agent skipped the failing test: FAILED
await film(3);
await page.evaluate(() => {                      // open it: what failed, and the test that was put back
  const d = document.querySelector('details.run'); if (d) d.open = true;
});
await film(4);
sh('git checkout -q tests && git apply ../countercheck-demo-fix.patch');
sh(`countercheck verify ${claim}`);              // the real fix: VERIFIED
await film(5);                                   // the page updates by itself; the FAILED run stays open below
await browser.close();
ui.kill();

execFileSync(ffmpeg, ['-y', '-loglevel', 'error', '-framerate', String(fps), '-i', path.join(frames, 'f%04d.png'),
  '-vf', 'scale=1000:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128[p];[b][p]paletteuse=dither=none',
  '-loop', '0', out]);
fs.rmSync(frames, { recursive: true, force: true });
console.log(`${n} frames -> ${out} (${Math.round(fs.statSync(out).size / 1024)} KB)`);
