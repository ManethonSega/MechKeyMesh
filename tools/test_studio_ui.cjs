// Full desktop shell regression. All hardware is mocked; no device is opened.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const { chromium } = require('playwright');
const root = path.resolve(__dirname, '..');
const frontend = path.join(root, 'apps/desktop/frontend/dist');
const output = path.resolve(process.env.STUDIO_UI_SCREENSHOTS || 'ui-review');
const layout = JSON.parse(fs.readFileSync(path.join(root, 'drivers/aula-f108pro/layouts/f108pro-ansi.json')));

async function installMock(page, failBoot = false) {
  await page.addInitScript(({ layout, failBoot }) => {
    const caps = ['keys', 'macros', 'hardware-rgb', 'per-key-rgb', 'live-rgb', 'music', 'display', 'clock', 'device-settings'];
    window.mock = { calls: [], unknown: [], connection: 'usb', caps, macros: [], rules: [], lightingPending: [], settingsPending: [], profiles: [{ id: 'default', name: 'Default', isDefault: true }, { id: 'gaming', name: 'Gaming', isDefault: false }] };
    const state = window.mock;
    window.__TAURI__ = { core: { invoke: async (command, args = {}) => {
      state.calls.push({ command, args: structuredClone(args) });
      switch (command) {
        case 'get_status': return { state: 'ready', candidateCount: 1, reconnectCount: 0, device: { name: 'AULA F108 Pro', driverId: 'aula.f108pro', connection: state.connection, capabilities: state.caps, vendorId: 0x0c45, productId: 0x800a, batteryPercent: 78 } };
        case 'get_lighting_effects':
          if (failBoot) throw Error('Test: controls unavailable');
          // Deliberately resolve after saved state could have loaded to guard startup ordering.
          await new Promise(resolve => setTimeout(resolve, 80));
          return [{ mode: 0, name: 'Off', color: 'none', brightness: false, speed: false, direction: null }, { mode: 1, name: 'Wave', color: 'color-or-rainbow', brightness: true, speed: true, direction: 'left-right' }];
        case 'get_desired_lighting': return { scope: { profileId: 'default' }, config: { mode: 1, r: 33, g: 230, b: 232, brightness: 5, speed: 3, direction: 1, colorful: false } };
        case 'get_persistence_status': return { schemaVersion: 7, integrityOk: true, journalMode: 'wal', path: 'C:\\Users\\Test\\MechKeyMesh\\settings.db' };
        case 'get_primary_layout': return layout;
        case 'get_keymap_capabilities': return { disable: true, keyboardKey: true, modifier: true, shortcut: true, mediaActions: ['volume-up','volume-down','mute'], mouseActions: ['left-click'], windowsActions: ['calculator'], macroAction: true };
        case 'get_desired_keymap': return { mappings: [] };
        case 'get_fn_layer_descriptor': return { modes: [{ mode: 'momentary', rawValue: 0 }], note: 'Write-only' };
        case 'get_per_key_rgb_capabilities': return { brightnessMin: 0, brightnessMax: 5, readbackSupported: true };
        case 'get_desired_per_key_rgb': return { brightness: 5, colors: [] };
        case 'get_macros': return structuredClone(state.macros);
        case 'get_macro_bank_descriptor': return { slotCount: 100, maximumBankBytes: 3584, maximumEventsPerMacro: 336, minimumDelayMilliseconds: 10, maximumDelayMilliseconds: 65535, minimumLoopCount: 1, maximumLoopCount: 99 };
        case 'save_macro': { const index = state.macros.findIndex(item => item.id === args.program.id); if (index < 0) state.macros.push(structuredClone(args.program)); else state.macros[index] = structuredClone(args.program); return args.program; }
        case 'delete_macro': state.macros = state.macros.filter(item => item.id !== args.macroId); return true;
        case 'get_device_settings_descriptor': return {
          sleepOptions: [{ timeout: 'never', label: 'Never' }, { timeout: 'five-minutes', label: '5 minutes' }],
          fnModes: [{ mode: 'momentary' }, { mode: 'toggle' }],
          keyResponseLevels: ['Fastest','Fast','Balanced','Stable','Maximum debounce'].map((label, index) => ({ level: index + 1, label, wiredTiming: `${index+1} ms`, wirelessTiming: `${index+2} ms`, bluetoothTiming: `${index+3} ms` })),
          note: 'OEM mappings are documented. Physical verification remains pending.'
        };
        case 'get_desired_device_settings': return { disableWindowsKey: false, disableAltTab: false, disableAltF4: false, fnMode: 'momentary', sleepTimeout: 'five-minutes', keyResponseLevel: 3 };
        case 'get_live_rgb_defaults': return { name: 'Evening wave', framesPerSecond: 30, brightness: .8, layers: [{ id: 'wave', name: 'Wave', kind: 'animation', effect: 'wave', enabled: true, opacity: 1, blendMode: 'replace', mask: [], order: 0, primary: {r:33,g:230,b:232}, secondary: {r:28,g:141,b:255}, speed: 1, width: 1 }] };
        case 'get_live_rgb_status': return { running: false, transport: { bufferedFrames: 0, sentFrames: 0, coalescedFrames: 0, failedFrames: 0 } };
        case 'get_music_defaults': return { mode: 'spectrum', backgroundMode: 'off', sensitivity: 1.25, smoothing: .45, falloff: .82, bass: 1.15, mids: 1, treble: 1, brightness: .85, palette: [{r:0,g:210,b:255},{r:117,g:67,b:255},{r:255,g:45,b:149},{r:255,g:190,b:0}] };
        case 'get_audio_output_devices': return [{ id: 'speakers', name: 'Speakers', isDefault: true }];
        case 'get_music_status': return { running: false, framesAnalyzed: 0, framesRendered: 0, transport: { coalescedFrames: 0 } };
        case 'get_lcd_transfer_status': return { totalPages: 0, pagesSent: 0, active: false, incomplete: false };
        case 'get_lcd_editor_state': return null;
        case 'get_profiles': return state.profiles;
        case 'get_application_rules': return state.rules;
        case 'get_profile_runtime_status': return { activeProfileId: 'default', autoSwitch: false, foregroundExecutable: 'MechKeyMesh.exe', lastReport: null };
        case 'get_native_observations': return { connection: state.connection, batteryPercent: state.connection === 'usb' ? null : 78, charging: null, knobPolicy: 'Native knob and menu navigation remain under firmware control.', evidenceNote: 'Charging and firmware readback are unavailable.' };
        case 'get_background_status': return { paused: false, preferences: { minimizeToTray: true, startMinimized: false }, startWithWindows: false };
        case 'get_logs': return [];
        case 'get_driver_registry': return [];
        case 'reconnect': return true;
        case 'select_lcd_media_file': return null;
        case 'select_profile_application_file': return 'C:\\Program Files\\Example\\Example.exe';
        case 'save_application_rule': state.rules.push(args.rule); return true;
        case 'delete_application_rule': state.rules = state.rules.filter(item => item.id !== args.id); return true;
        case 'delete_profile': state.profiles = state.profiles.filter(item => item.id !== args.profileId); return true;
        case 'apply_lighting': return new Promise((resolve, reject) => state.lightingPending.push({ resolve, reject }));
        case 'save_device_settings': return new Promise((resolve, reject) => state.settingsPending.push({ resolve, reject }));
        default: state.unknown.push(command); throw Error('Unexpected mock command: ' + command);
      }
    } } };
  }, { layout, failBoot });
}
async function navigate(page, name) {
  await page.locator(`#nav [data-page="${name}"]`).click();
  assert.equal(await page.locator(`#page-${name}`).isVisible(), true);
  assert.equal(await page.locator(`#nav [data-page="${name}"]`).getAttribute('aria-current'), 'page');
  assert.equal(await page.locator('#pageTitle').evaluate(el => el === document.activeElement), true);
}
async function checkLayout(page, name) {
  const dimensions = await page.evaluate(() => {
    const main = document.querySelector('main');
    const nav = document.querySelector('#nav');
    const version = document.querySelector('.version').getBoundingClientRect();
    const unlabeled = [...document.querySelectorAll('.page.active input,.page.active select,.page.active textarea')].filter(el => el.getClientRects().length && !el.getAttribute('aria-label') && !el.getAttribute('aria-labelledby') && !el.labels?.length).map(el => el.id || el.className);
    return { extraWidth: main.scrollWidth-main.clientWidth, navBottom: nav.getBoundingClientRect().bottom, versionTop: version.top, versionBottom: version.bottom, height: innerHeight, unlabeled };
  });
  assert(dimensions.extraWidth <= 2, `${name}: horizontal overflow ${JSON.stringify(dimensions)}`);
  assert(dimensions.navBottom <= dimensions.versionTop + 2 && dimensions.versionBottom <= dimensions.height, `${name}: sidebar clipped`);
  assert.deepEqual(dimensions.unlabeled, [], `${name}: controls need names`);
}
async function screenshot(page, name) { await page.screenshot({ path: path.join(output, `${name}.png`) }); }

(async () => {
  fs.mkdirSync(output, { recursive: true });
  const server = http.createServer((req, res) => {
    const urlPath = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    const file = path.join(frontend, urlPath === '/' ? 'index.html' : urlPath);
    if (!file.startsWith(frontend + path.sep)) { res.writeHead(403).end(); return; }
    fs.readFile(file, (error, content) => {
      if (error) { res.writeHead(404).end(); return; }
      const type = { '.html':'text/html', '.js':'text/javascript', '.css':'text/css', '.png':'image/png' }[path.extname(file)] || 'text/plain';
      res.writeHead(200, { 'Content-Type': type }); res.end(content);
    });
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const url = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE || chromium.executablePath() });
  try {
    const page = await browser.newPage({ viewport: { width: 1320, height: 840 }, colorScheme: 'dark' });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await installMock(page);
    await page.goto(url);
    await page.waitForSelector('#app:not(.hidden)');
    await page.waitForFunction(() => document.querySelector('#keymapStatus').textContent.includes('saved') && document.querySelector('#deviceFnMode').options.length === 2);
    assert.equal(await page.locator('#lightingMode').inputValue(), '1', 'saved mode restored after async options');
    assert.equal(await page.locator('#direction').inputValue(), '1', 'saved direction preserved');
    assert.equal(await page.evaluate(() => mock.calls.some(call => ['apply_lighting','save_device_settings','upload_macros','upload_lcd_asset'].includes(call.command))), false, 'startup must not write hardware');
    const pages = ['home','keys','macros','lighting','live-rgb','music','lcd','profiles','settings','knob','background','diagnostics'];
    for (const theme of ['dark','light']) {
      await page.evaluate(theme => applyTheme(theme), theme);
      for (const name of pages) { await navigate(page,name); await checkLayout(page,`${name}-${theme}`); await screenshot(page,`${name}-${theme}`); }
    }
    // Both native pickers remain wired to the desktop commands, and cancellation is harmless.
    await navigate(page, 'lcd');
    await page.locator('#lcdImport').click();
    await page.waitForFunction(() => document.querySelector('#lcdStatus').textContent === 'No media file selected.');
    assert.equal(await page.locator('#lcdMediaPath').getAttribute('readonly'), '');
    assert.equal(await page.evaluate(() => mock.calls.some(call => call.command === 'import_lcd_media')), false);
    await navigate(page, 'profiles');
    assert.match(await page.locator('#ruleList').textContent(), /No apps linked yet/);
    await page.locator('#ruleBrowse').click();
    await page.waitForFunction(() => document.querySelector('#rulePattern').value.includes('Example.exe'));
    await page.locator('#ruleAdd').click();
    await page.waitForSelector('.rule-row');
    await page.locator('.rule-row button').click();
    await page.locator('#confirmDialog button[value="cancel"]').click();
    assert.equal(await page.locator('.rule-row').count(), 1);
    await page.locator('.rule-row button').click();
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('.rule-row').count(), 1);
    await page.locator('.rule-row button').click();
    await page.locator('#confirmDialog button[value="confirm"]').click();
    await page.waitForSelector('.empty-state');
    // Navigation and keyboard tabs do not discard an unsubmitted lighting draft.
    await navigate(page, 'lighting');
    await page.locator('#brightness').evaluate(el => { el.value='2'; el.dispatchEvent(new Event('input', { bubbles:true })); });
    assert.equal(await page.locator('#lightingSaveState').textContent(), 'Unsaved changes');
    await navigate(page, 'keys');
    await page.locator('[role="tab"][data-layer="normal"]').focus();
    await page.keyboard.press('ArrowRight');
    assert.equal(await page.locator('[data-layer="fn"]').getAttribute('aria-selected'), 'true');
    await page.keyboard.press('Home');
    assert.equal(await page.locator('[data-layer="normal"]').getAttribute('aria-selected'), 'true');
    await navigate(page, 'lighting');
    assert.equal(await page.locator('#brightness').inputValue(), '2');
    await page.locator('#applyLighting').click();
    await page.waitForFunction(() => mock.lightingPending.length === 1);
    await page.evaluate(() => updateStatus({ state: 'ready', device: { driverId:'aula.f108pro', connection:'usb', capabilities: mock.caps } }));
    assert.equal(await page.locator('#applyLighting').isDisabled(), true, 'polling must not unlock an in-flight save');
    assert.equal(await page.locator('#brightness').isDisabled(), true);
    await page.evaluate(() => mock.lightingPending.shift().resolve({ applied: false, message:'Device disconnected. Reconnect to apply.' }));
    await page.waitForFunction(() => !document.querySelector('#applyLighting').disabled);
    assert.equal(await page.locator('#lightingSaveState').getAttribute('data-state'), 'pending');
    await page.locator('#brightness').evaluate(el => { el.value='3'; el.dispatchEvent(new Event('input', { bubbles:true })); });
    await page.locator('#applyLighting').click();
    await page.waitForFunction(() => mock.lightingPending.length === 1);
    await page.evaluate(() => mock.lightingPending.shift().reject(Error('Test timeout')));
    await page.waitForFunction(() => document.querySelector('#lightingSaveState').dataset.state === 'error');
    assert.equal(await page.locator('#brightness').inputValue(), '3', 'failed saves retain edits');
    await page.locator('#applyLighting').click();
    await page.waitForFunction(() => mock.lightingPending.length === 1);
    await page.evaluate(() => mock.lightingPending.shift().resolve({ applied: true }));
    await page.waitForFunction(() => document.querySelector('#lightingSaveState').dataset.state === 'sent');
    await screenshot(page,'lighting-sent-light');
    await page.evaluate(() => document.querySelector('main').scrollTo(0,document.querySelector('main').scrollHeight));
    await screenshot(page,'lighting-editor-light');
    await navigate(page, 'settings');
    await page.locator('#disableWindowsKey').check();
    assert.equal(await page.locator('#settingsSaveState').textContent(), 'Unsaved changes');
    await page.locator('#applyDeviceSettings').click();
    await page.waitForFunction(() => mock.settingsPending.length === 1);
    assert.equal(await page.locator('#disableWindowsKey').isDisabled(), true);
    await page.evaluate(() => mock.settingsPending.shift().resolve({ applied: false, message:'Test: disconnected' }));
    await page.waitForFunction(() => document.querySelector('#settingsSaveState').dataset.state === 'pending');
    assert.equal(await page.locator('#disableWindowsKey').isChecked(), true);
    // Delete is cancellable. No macro upload is triggered by local CRUD.
    await navigate(page, 'macros');
    await page.locator('#newMacro').click();
    await page.waitForFunction(() => mock.macros.length === 1 && !document.querySelector('#deleteMacro').disabled);
    await page.locator('#deleteMacro').click();
    await page.locator('#confirmDialog button[value="cancel"]').click();
    assert.equal(await page.evaluate(() => mock.macros.length), 1);
    await page.locator('#deleteMacro').click();
    await page.locator('#confirmDialog button[value="confirm"]').click();
    await page.waitForFunction(() => mock.macros.length === 0);
    // The smallest supported desktop window remains operable in both themes.
    await page.setViewportSize({ width:1040, height:680 });
    for (const theme of ['dark','light']) {
      await page.evaluate(theme => applyTheme(theme), theme);
      for (const name of ['home','keys','macros','lighting','lcd','profiles','settings','live-rgb']) { await navigate(page,name); await checkLayout(page,`${name}-small-${theme}`); await screenshot(page,`${name}-small-${theme}`); }
    }
    await page.evaluate(() => document.documentElement.style.fontSize = '20px');
    for (const name of ['home','profiles','settings']) { await navigate(page,name); await checkLayout(page,`${name}-larger-text`); }
    await page.evaluate(() => document.documentElement.style.fontSize = '');
    await page.emulateMedia({ reducedMotion:'reduce' });
    assert.equal(await page.locator('#themeToggle').evaluate(el => getComputedStyle(el).transitionDuration), '0s');
    // Wireless capability changes hide unsupported destinations and return to Overview.
    await navigate(page,'live-rgb');
    await page.evaluate(() => { mock.connection='wireless24-g'; mock.caps=['keys','macros','hardware-rgb','per-key-rgb','device-settings','battery']; });
    await page.evaluate(() => pollStatus());
    assert.equal(await page.locator('#page-home').isVisible(), true);
    assert.equal(await page.locator('#nav [data-page="live-rgb"]').isVisible(), false);
    assert.equal(await page.locator('#nav [data-page="music"]').isVisible(), false);
    assert.equal(await page.locator('#batteryPercent').textContent(), '78%');
    assert.equal(await page.locator('#readPerKeyHardware').isDisabled(), true);
    await page.evaluate(() => { mock.connection='usb'; mock.caps=['keys','macros','hardware-rgb','per-key-rgb','live-rgb','music','display','clock','device-settings']; });
    await page.evaluate(() => pollStatus());
    assert.equal(await page.locator('#nav [data-page="live-rgb"]').isVisible(), true);
    assert.equal(await page.locator('#batteryMetric').isVisible(), false);
    assert.deepEqual(await page.evaluate(() => mock.unknown), []);
    assert.deepEqual(errors, []);
    const failed = await browser.newPage();
    await installMock(failed,true);
    await failed.goto(url);
    await failed.waitForSelector('#app:not(.hidden)');
    assert.equal(await failed.locator('#bootNotice').isVisible(), true, 'startup failure must not trap user on splash');
    await failed.close();
    console.log('Studio UI passed: all 12 pages, two themes, two desktop sizes, labels, draft/send/failure feedback, asynchronous-save locking, confirmations, native pickers, keyboard tabs, reduced motion, startup recovery and USB/wireless capabilities. Hardware mocked throughout.');
  } finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error); process.exitCode=1; });
