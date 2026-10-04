// P0-1 直接验证：走真实 WS onmessage 分支，而非绕道直接调 loadTasks()。
//
// 背景（HANDOFF.md 记录的缺口）：
//   原测试只调 t.loadTasks()，改动的代码在 ws.onmessage 里。两者逻辑等价
//   （onmessage 最终也是调 loadTasks()），但严格说只是间接验证。
//   本文件通过给 connectWs() 打桩，拿到它注册的 onmessage 真身并直接驱动，
//   覆盖到 onmessage 的快照分支判断、重载触发、非数组/坏 JSON 的边界。
//
// 运行： node test-ws.js
const H = require('./harness.cjs');
let pass = 0, fail = 0;
function ok(cond, name, detail) {
  if (cond) { pass++; console.log('  PASS ' + name); }
  else { fail++; console.log('  FAIL ' + name + (detail ? '  <- ' + detail : '')); }
}

function page1Tasks() {
  return Array.from({length: 50}, (_, i) => ({
    chapterId: 'ch-' + String(60 - i).padStart(3,'0'), comicId: 'c1',
    chapterTitle: '第 ' + (60-i) + ' 话', state: 'pending',
    doneImgCount: 0, totalImgCount: 10, progress: 0, retryCount: 0,
  }));
}
function page2Tasks() {
  return Array.from({length: 10}, (_, i) => ({
    chapterId: 'ch-0' + (10 - i), comicId: 'c1', chapterTitle: '第 ' + (10-i) + ' 话',
    state: 'pending', doneImgCount: 0, totalImgCount: 10, progress: 0, retryCount: 0,
  }));
}

let calls = [];
function makeFetch(table) {
  return async (url) => {
    calls.push(url);
    const body = table(url);
    return { ok: true, status: 200, statusText: 'OK', text: async () => JSON.stringify(body) };
  };
}

// 每个用例前重置 WS 桩，返回一个「可手动触发回调」的实例
function wsStub() {
  H.setWs(() => ({ onopen: null, onclose: null, onerror: null, onmessage: null,
    close(){}, send(){}, readyState: 1 }));
}

// 让 onmessage 里 await 的 loadTasks() 落地（两轮微任务足够）
async function settle() {
  await new Promise(r => setImmediate(r));
  await new Promise(r => setImmediate(r));
}

(async () => {
  console.log('== P0-1（直接）: WS onmessage 收到快照后不覆盖分页 ==');
  {
    wsStub();
    H.setFetch(makeFetch(u => u.includes('offset=50')
      ? { tasks: page2Tasks(), total: 60 }
      : { tasks: page1Tasks(), total: 60 }));
    const t = H.load();

    // 走真实路径：调 connectWs，拿到它注册的 onmessage
    t.connectWs();
    const ws = H.lastWs();
    ok(!!ws, 'connectWs 创建了 WebSocket 实例');
    ok(typeof ws.onmessage === 'function', 'onmessage 已注册');

    // 先在第 2 页
    t.state.pageOffset = 50;
    t.state.totalTasks = 60;
    await t.loadTasks();
    ok(t.state.tasks.length === 10, '第 2 页 10 条', 'got ' + t.state.tasks.length);

    // 触发真实 onmessage —— 全量 60 条快照
    calls = [];
    ws.onmessage({ data: JSON.stringify({
      topic: 'task-snapshot-event',
      payload: page1Tasks().concat(page2Tasks()),   // 全量 60 条
    }) });
    await settle();

    ok(calls.length >= 1, 'onmessage 确实触发了重新加载', 'calls=' + calls.length);
    ok((calls[0] || '').includes('offset=50'),
       '重新加载仍请求 offset=50（未被快照改成 offset=0）', calls[0]);
    ok(t.state.tasks.length === 10,
       '列表仍是 10 条，未被 60 条快照撑爆', 'got ' + t.state.tasks.length);
  }

  console.log('== P0-1c（直接）: WS 快照不冲掉筛选 ==');
  {
    wsStub();
    H.setFetch(makeFetch(u => u.includes('state=failed')
      ? { tasks: [{chapterId:'ch-f1', state:'failed', comicId:'c1', chapterTitle:'x', doneImgCount:1, totalImgCount:2}], total: 1 }
      : { tasks: page1Tasks(), total: 60 }));
    const t = H.load();
    t.connectWs();
    const ws = H.lastWs();

    t.state.filterState = 'failed';
    await t.loadTasks();
    ok(t.state.tasks.length === 1 && t.state.tasks[0].chapterId === 'ch-f1', '筛选生效 1 条');

    calls = [];
    ws.onmessage({ data: JSON.stringify({
      topic: 'task-snapshot-event',
      payload: page1Tasks().concat(page2Tasks()),
    }) });
    await settle();

    ok((calls[0] || '').includes('state=failed'),
       '快照后重新加载仍带 state=failed', calls[0]);
    ok(t.state.tasks.length === 1, '筛选结果未被全量冲掉', 'got ' + t.state.tasks.length);
  }

  console.log('== P0-1d: 非数组 payload 不触发加载 ==');
  {
    wsStub();
    H.setFetch(makeFetch(() => ({ tasks: page1Tasks(), total: 60 })));
    const t = H.load();
    t.connectWs();
    const ws = H.lastWs();
    await t.loadTasks();

    calls = [];
    ws.onmessage({ data: JSON.stringify({ topic: 'task-snapshot-event', payload: { not: 'array' } }) });
    await settle();
    ok(calls.length === 0, '非数组 payload 不触发加载', 'calls=' + calls.length);
  }

  console.log('== 解析失败不应崩 ==');
  {
    wsStub();
    H.setFetch(makeFetch(() => ({ tasks: page1Tasks(), total: 60 })));
    const t = H.load();
    t.connectWs();
    const ws = H.lastWs();
    calls = [];
    let threw = false;
    try { ws.onmessage({ data: '{ 坏 JSON' }); } catch (e) { threw = true; }
    ok(!threw, '坏 JSON 不抛异常');
    ok(calls.length === 0, '坏 JSON 不触发加载');
  }

  console.log('');
  console.log('== 结果: ' + pass + ' passed, ' + fail + ' failed ==');
  process.exit(fail > 0 ? 1 : 0);
})();