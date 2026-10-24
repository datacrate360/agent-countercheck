// Records docs/assets/demos/ui.gif: the live `countercheck ui` page while three agents work in one folder.
// Real runs of the countercheck on your PATH on the demo project from setup-team.sh:
//   1. "Discount fix" (claude-code) skipped the failing test: checking now -> FAILED, sent back
//   2. the code is fixed; "Docs" (codex) and "Refactor" (hermes) run at the same time -> both delivered
//   3. point at the failed agent (its path and the check it broke light up), open its run: the replay
//   4. "Discount fix" runs again -> VERIFIED; all three delivered
// Needs: node, Google Chrome, ffmpeg, and puppeteer-core (`npm i -g puppeteer-core`, or NODE_PATH=<a node_modules>).
// Usage: node docs/assets/demos/record-ui.mjs        (THEME=dark for the dark page, OUT=<name>.gif)
import { createRequire } from 'node:module';
import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const require = createRequire(import.meta.url);
const puppeteer = require('puppeteer-core');
const here = path.dirname(new URL(import.meta.url).pathname);
const out = path.join(here, process.env.OUT || 'ui.gif');
const chrome = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const ffmpeg = process.env.FFMPEG || 'ffmpeg';
const demo = '/tmp/countercheck-team';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const sh = (cmd) => execFileSync('sh', ['-c', cmd], { cwd: demo, stdio: 'ignore' });
// A run in the background, so the page can show it while it is checking.
const bg = (args) => new Promise((resolve) => {
  spawn('countercheck', ['verify', ...args], { cwd: demo, stdio: 'ignore' }).on('exit', resolve);
});

execFileSync('sh', [path.join(here, 'setup-team.sh')], { stdio: 'ignore' });
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
try {
  const page = await browser.newPage();
  await page.emulateMediaFeatures([{ name: 'prefers-color-scheme', value: process.env.THEME || 'light' }]);
  await page.setViewport({ width: 1100, height: 680, deviceScaleFactor: 1 });
  await page.goto(url, { waitUntil: 'networkidle0' });

  // Where the camera points: an element's top. The page reloads itself when a run lands, so
  // this is re-applied on every frame.
  let target = '.totals';
  const aim = () => page.evaluate((sel) => {
    const el = document.querySelector(sel);
    if (el) window.scrollTo(0, el.getBoundingClientRect().top + window.scrollY - 24);
  }, target).catch(() => {});
  let n = 0;
  const fps = 6;
  const film = async (seconds) => {
    for (let i = 0; i < seconds * fps; i++) {
      await aim();
      try {
        await page.screenshot({ path: path.join(frames, `f${String(n).padStart(4, '0')}.png`) });
        n++;
      } catch {} // mid-reload: skip the frame
      await sleep(1000 / fps);
    }
  };

  await film(1.5);                                         // no runs yet
  let a = bg(['--agent', 'claude-code', '--model', 'claude-opus-5', '--role', 'Discount fix',
    '--claim', 'Fixed the discount bug. All tests pass.']);
  target = '.flow';
  await film(3);                                           // checking now: a moving line
  await a;
  await film(2.5);                                         // FAILED, sent back
  sh('git checkout -q tests && git apply ../countercheck-team-fix.patch');
  a = Promise.all([
    bg(['--agent', 'codex', '--model', 'gpt-6.1', '--role', 'Docs', '--claim', 'Updated the pricing examples.']),
    bg(['--agent', 'hermes', '--model', 'deepseek-v4-pro', '--role', 'Refactor',
      '--claim', 'Tidied pricing.py, no behaviour change.']),
  ]);
  await film(3);                                           // two agents checking at once
  await a;
  await film(2.5);                                         // both delivered
  const failed = await page.$('.flow label.failed');
  if (failed) await failed.hover();
  await film(2.5);                                         // its path and the check it broke light up
  await page.mouse.move(5, 5);
  target = 'details.run.v-failed';
  await page.evaluate(() => { const d = document.querySelector('details.run.v-failed'); if (d) d.open = true; });
  await film(5);                                           // the replay: locked, clean copy, put back, checks, sealed
  a = bg(['--agent', 'claude-code', '--model', 'claude-opus-5', '--role', 'Discount fix',
    '--claim', 'Fixed the discount bug. All tests pass.']);
  target = '.flow';
  await film(2);
  await a;
  await film(3);                                           // all three delivered

  execFileSync(ffmpeg, ['-y', '-loglevel', 'error', '-framerate', String(fps), '-i', path.join(frames, 'f%04d.png'),
    '-vf', 'scale=880:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle',
    '-loop', '0', out]);
  console.log(`${n} frames -> ${out} (${Math.round(fs.statSync(out).size / 1024)} KB)`);
} finally {
  await browser.close();
  ui.kill();
  fs.rmSync(frames, { recursive: true, force: true });
}
