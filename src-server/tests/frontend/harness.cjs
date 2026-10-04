// 单文件控制台（static/index.html）的测试桩。
//
// 直接从 index.html 抽取 <script> 块执行，不依赖任何复制出来的副本——
// 副本会随源码改动而失效，那样测试就变成「验证旧代码」。
const fs = require('fs');
const path = require('path');

const HTML = path.join(__dirname, '..', '..', 'static', 'index.html');

function extractScript(html) {
  // 取最后一个 <script>...</script>（无 src 属性），即内联控制台逻辑
  const m = html.match(/<script>([\s\S]*?)<\/script>/g);
  if (!m || !m.length) throw new Error('index.html 里没找到内联 <script> 块');
  const last = m[m.length - 1];
  return last.replace(/^<script>/, '').replace(/<\/script>$/, '');
}

const written = {};

function makeEl(id) {
  const el = {
    id, _innerHTML: '', _textContent: '', value: '', checked: false,
    disabled: false, type: 'text', childElementCount: 0, firstChild: null,
    classList: { toggle(){}, add(){}, remove(){}, contains(){ return false; } },
    dataset: {},
    setAttribute(){}, getAttribute(){ return null; },
    _kids: [],
    appendChild(c){ this._kids.push(c); this.childElementCount = this._kids.length; this.firstChild = this._kids[0] || null; return c; },
    removeChild(c){ const i = this._kids.indexOf(c); if (i >= 0) this._kids.splice(i,1); this.childElementCount = this._kids.length; this.firstChild = this._kids[0] || null; return c; },
    remove(){}, addEventListener(){},
    querySelectorAll(){ return []; }, querySelector(){ return null; },
  };
  Object.defineProperty(el, 'innerHTML', {
    get(){ return el._innerHTML; },
    set(v){ el._innerHTML = String(v); written[id] = el._innerHTML; },
  });
  Object.defineProperty(el, 'textContent', {
    get(){ return el._textContent; },
    set(v){ el._textContent = String(v); written[id] = el._textContent; },
  });
  return el;
}

const els = {};
function $(id) { if (!els[id]) els[id] = makeEl(id); return els[id]; }

global.document = {
  getElementById: $,
  querySelectorAll: () => [],
  createElement: () => makeEl('_created'),
  addEventListener: () => {},
};
global.location = { host: '127.0.0.1:8081', protocol: 'http:' };
global.window = global;

// 定时器：测试里不让真实重连/轮询跑起来
global.setInterval = () => 0;
global.setTimeout = () => 0;
global.clearTimeout = () => {};
global.clearInterval = () => {};
global.confirm = () => true;
global.prompt = () => null;
global.alert = () => {};

// WebSocket 桩：每个用例通过 setWs() 指定工厂，lastWs() 取回实例
let wsFactory = () => { throw new Error('WS 未在本用例中打桩，请先调 setWs()'); };
let lastWsInstance = null;
global.WebSocket = function(...args) {
  lastWsInstance = wsFactory(...args);
  return lastWsInstance;
};
function setWs(fn) { wsFactory = fn; lastWsInstance = null; }
function lastWs() { return lastWsInstance; }

// fetch 桩
let fetchImpl = null;
global.fetch = (...args) => fetchImpl(...args);
function setFetch(fn){ fetchImpl = fn; }

// 载入被测代码。每次调用都重新 eval，得到互相隔离的 state。
const code = extractScript(fs.readFileSync(HTML, 'utf8'));
const EXPORTS = ['state','loadTasks','renderTasks','renderTasksError','applyTaskEvent',
                 'buildQuery','esc','PAGE_SIZE','connectWs'];

const wrapped = code
  + '\n;globalThis.__t = {' + EXPORTS.join(', ') + '};';

module.exports = {
  written, $, els, setFetch, setWs, lastWs, extractScript,
  load: () => { eval(wrapped); return globalThis.__t; },
};