// 注入回归：chapterId 含引号时，任务行按钮不得逃逸出 JS 字符串字面量。
//
// 背景：renderTasks 原先内联拼 onclick="taskAction('${esc(t.chapterId)}','pause')"。
//   esc() 只做 HTML 实体转义（' -> &#39;），不做 JS 字符串转义。
//   浏览器解析 HTML 属性时会把 &#39; 还原成 '，于是值里的单引号能闭合字面量，
//   后续内容被当代码执行 —— 这是真实的注入路径。
//   chapterId 在后端是 String（responses/mod.rs:33 SeriesRespData.id），无格式约束。
//
// 修复方式：改 data-* 传值 + 事件委托，值只经 dataset 读取，不进 JS 源码。
// 本文件断言两件事：
//   1. 渲染结果里不再出现内联 onclick；
//   2. 委托监听派发时，取到的 chapterId 与原始值逐字节相等（未被 HTML 实体污染）。
//
// 运行： node test-injection.cjs
const H = require('./harness.cjs');
let pass = 0, fail = 0;
function ok(cond, name, detail) {
  if (cond) { pass++; console.log('  PASS ' + name); }
  else { fail++; console.log('  FAIL ' + name + (detail ? '  <- ' + detail : '')); }
}

const NASTY = "ch'1\";alert(1)//";

function decodeEnt(s) {
  return s.replace(/&#39;/g, "'").replace(/&quot;/g, '"')
          .replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
}

(async () => {
  console.log('== 注入：含引号的 chapterId 不逃逸 ==');
  {
    H.setWs(() => ({ onopen:null, onclose:null, onerror:null, onmessage:null, close(){}, send(){}, readyState:1 }));
    H.setFetch(async () => ({ ok:true, status:200, statusText:'OK', text: async () => JSON.stringify({
      tasks: [{
        chapterId: NASTY, comicId: 'c1', chapterTitle: 'x', state: 'downloading',
        doneImgCount: 1, totalImgCount: 2, progress: 50, retryCount: 0,
      }], total: 1 }) }));
    const t = H.load();
    await t.loadTasks();
    t.renderTasks();

    const html = H.$('task-rows').innerHTML;
    ok(!/onclick\s*=/.test(html), '渲染结果不含内联 onclick', html.slice(0, 200));
    ok(html.includes('data-act="pause"'), '改用 data-act 传动作');
    ok(html.includes('data-cid='), '改用 data-cid 传 chapterId');

    const m = html.match(/data-cid="([^"]*)"/);
    ok(!!m, '能取到 data-cid 属性值');
    if (m) {
      const decoded = decodeEnt(m[1]);
      ok(decoded === NASTY, 'data-cid 解码后与原始 chapterId 相等', JSON.stringify(decoded));
    }
  }

  console.log('== 注入：委托监听按 data-act 正确分发 ==');
  {
    H.setWs(() => ({ onopen:null, onclose:null, onerror:null, onmessage:null, close(){}, send(){}, readyState:1 }));
    const seen = [];
    H.setFetch(async (url, opt) => {
      seen.push({ url, opt });
      return { ok:true, status:200, statusText:'OK', text: async () => JSON.stringify({ tasks: [], total: 0 }) };
    });
    const t = H.load();
    t.bindTaskActions();

    const btn = { dataset: { act: 'delete', cid: NASTY } };
    const ev = { target: { closest: (sel) => (sel === 'button[data-act]' ? btn : null) } };
    H.$('task-rows')._fire('click', ev);

    await new Promise(r => setImmediate(r));
    // 注意：不能只匹配 '/api/tasks/'——deleteTask 成功后调 refreshAll()，
    // 其中 loadStats() 打的是 '/api/tasks/stats'，也含该子串，会误抓。
    const del = seen.find(s => /\/api\/tasks\/(?!stats)/.test(s.url || ''));
    ok(!!del, 'delete 动作被派发', JSON.stringify(seen.map(s => s.url)));
    if (del) {
      const raw = del.url.split('/api/tasks/')[1].split('?')[0];
      const idPart = decodeURIComponent(raw);
      ok(idPart === NASTY, '委托派发时 chapterId 与原始值相等', JSON.stringify(idPart));
    }
  }

  console.log('');
  console.log('== 结果: ' + pass + ' passed, ' + fail + ' failed ==');
  process.exit(fail > 0 ? 1 : 0);
})();
