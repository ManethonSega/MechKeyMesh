const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');

async function runChecks(page, frontend, section) {
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.setContent('<html data-theme="dark"><body>' + section + '</body></html>');
    await page.addStyleTag({ content: fs.readFileSync(path.join(frontend, 'app.css'), 'utf8') + fs.readFileSync(path.join(frontend, 'macros.css'), 'utf8') + '.page {display:block!important}' });
    await page.evaluate(() => {
      window.saved = [];
      window.calls = [];
      window.pendingUploads = [];
      window.__TAURI__ = { core: { invoke: async (command, args = {}) => {
        window.calls.push(command);
        if (command === 'get_macros') return structuredClone(window.saved);
        if (command === 'get_macro_bank_descriptor') return { slotCount: 100, maximumBankBytes: 3584, maximumEventsPerMacro: 336, minimumDelayMilliseconds: 10, maximumDelayMilliseconds: 65535, minimumLoopCount: 1, maximumLoopCount: 99 };
        if (command === 'save_macro') {
          await new Promise(r => setTimeout(r, 10));
          if (!args.program.name.trim()) throw new Error('Macro name is required');
          const program = structuredClone(args.program);
          const index = window.saved.findIndex(p => p.id === program.id);
          if (index < 0) window.saved.push(program); else window.saved[index] = program;
          return program;
        }
        if (command === 'delete_macro') { window.saved = window.saved.filter(p => p.id !== args.macroId); return true; }
        if (command === 'upload_macros') return new Promise(resolve => window.pendingUploads.push(resolve));
        if (command === 'simulate_macro') return { steps: [], durationMilliseconds: 0, hardwareTouched: false };
        throw new Error('Unexpected command: ' + command);
      } } };
    });
    await page.addScriptTag({ content: fs.readFileSync(path.join(frontend, 'macros.js'), 'utf8') });
    await page.waitForFunction(() => document.querySelector('#macroLibraryStatus').textContent.includes('No saved macros'));
    assert.equal(await page.locator('#deleteMacro').isDisabled(), true);
    await page.locator('#newMacro').click();
    await page.waitForFunction(() => window.saved.length === 1 && !document.querySelector('#newMacro').disabled);
    assert.match(await page.locator('#macroLibrary').textContent(), /New macro 1/);
    await page.locator('#macroName').fill('Example');
    await page.locator('#insertMacroEventButton').click();
    await page.locator('#duplicateMacro').click();
    await page.waitForFunction(() => window.saved.length === 2 && !document.querySelector('#newMacro').disabled);
    assert.equal(await page.evaluate(() => window.saved.find(p => p.name === 'Example copy').events.length), 1);
    const originalId = await page.evaluate(() => window.saved.find(p => p.name === 'Example').id);
    await page.locator('#macroLibrary').selectOption(originalId);
    assert.equal(await page.locator('#macroTimeline tr').count(), 1);
    await page.locator('#newMacro').click();
    await page.waitForFunction(() => window.saved.length === 3 && !document.querySelector('#newMacro').disabled);
    assert.equal(JSON.parse(await page.locator('#macroJson').inputValue()).events.length, 0);
    await page.locator('#deleteMacro').click();
    await page.waitForFunction(() => window.saved.length === 2 && !document.querySelector('#newMacro').disabled);
    assert(!await page.evaluate(() => window.calls.includes('upload_macros')));
    await page.locator('#uploadMacros').click();
    await page.waitForFunction(() => window.pendingUploads.length === 1);
    await page.locator('#newMacro').click();
    await page.waitForFunction(() => window.saved.length === 3 && !document.querySelector('#newMacro').disabled);
    assert.equal(await page.locator('#uploadMacros').isDisabled(), true);
    await page.evaluate(() => window.pendingUploads[0]({ applied: false, message: 'simulated USB timeout' }));
    await page.waitForFunction(() => document.querySelector('#macroLibraryStatus').textContent.includes('simulated USB timeout'));
    assert.equal(await page.locator('#newMacro').isDisabled(), false);
    assert.equal(await page.locator('#uploadMacros').isDisabled(), false);
    await page.evaluate(() => macroLoadLibrary());
    assert.equal(await page.evaluate(() => window.saved.length), 3);
    assert.equal(await page.locator('#macroLibrary option:not(:disabled)').count(), 3);
    assert.deepEqual(errors, []);
    if (process.env.MACRO_UI_SCREENSHOT) await page.screenshot({ path: process.env.MACRO_UI_SCREENSHOT, fullPage: true });
    console.log('Macro UI: create, save, copy, select, delete, reload and editing during a pending/failed hardware upload passed.');
}

module.exports = { runChecks };

if (require.main === module) (async () => {
  const root = path.resolve(__dirname, '..');
  const frontend = path.join(root, 'apps/desktop/frontend/dist');
  const html = fs.readFileSync(path.join(frontend, 'index.html'), 'utf8');
  const section = html.slice(html.indexOf('<section id="page-macros"'), html.indexOf('<section id="page-lighting"'));
  const browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || chromium.executablePath() });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
    await runChecks(page, frontend, section);
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
