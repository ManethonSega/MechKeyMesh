const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { JSDOM } = require(process.env.JSDOM_MODULE || 'jsdom');
const { runChecks } = require('./test_macro_ui.cjs');

(async () => {
  const frontend = path.resolve(__dirname, '../apps/desktop/frontend/dist');
  const html = fs.readFileSync(path.join(frontend, 'index.html'), 'utf8');
  const section = html.slice(html.indexOf('<section id="page-macros"'), html.indexOf('<section id="page-lighting"'));
  let dom;
  const errors = [];
  const evaluate = (fn, arg) => dom.window.eval('(' + fn.toString() + ')(' + (arg === undefined ? '' : JSON.stringify(arg)) + ')');
  const page = {
    on: (event, listener) => { if (event === "pageerror") errors.push(listener); },
    setContent: async content => {
      dom = new JSDOM(content, { runScripts: 'outside-only', url: 'https://macro.test' });
      dom.window.structuredClone = structuredClone;
      dom.window.confirm = () => true;
      dom.window.addEventListener('error', e => errors.forEach(listener => listener(e.error)));
    },
    addStyleTag: async () => {},
    addScriptTag: async ({ content }) => dom.window.eval(content),
    evaluate,
    waitForFunction: async fn => {
      const deadline = Date.now() + 5000;
      while (!evaluate(fn)) {
        if (Date.now() > deadline) throw new Error('DOM condition timed out: ' + fn.toString());
        await new Promise(r => setTimeout(r, 5));
      }
    },
    locator: selector => {
      const get = () => dom.window.document.querySelector(selector);
      return {
        click: async () => { assert(get() && !get().disabled); get().click(); },
        fill: async value => { get().value = value; get().dispatchEvent(new dom.window.Event('input', { bubbles: true })); },
        selectOption: async value => { get().value = value; get().dispatchEvent(new dom.window.Event('change', { bubbles: true })); },
        isDisabled: async () => get().disabled,
        textContent: async () => get().textContent,
        inputValue: async () => get().value,
        count: async () => dom.window.document.querySelectorAll(selector).length,
      };
    },
  };
  try { await runChecks(page, frontend, section); }
  finally { dom?.window.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });

