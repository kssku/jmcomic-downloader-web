// P0 行为测试：分页 / 失败区分空态 / 竞态 / 就地更新
// 针对 src-server/static/index.html 里嵌的控制台脚本。
// 运行：node src-server/tests/frontend/test-behavior.cjs
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

(async () => {
  console.log('== P0-1: 分页不被全量撑爆 ==');
  {
    H.setFetch(makeFetch(u => u.includes('offset=50')
      ? { tasks: page2Tasks(), total: 60 }
      : { tasks: page1Tasks(), total: 60 }));
    const t = H.load();
    t.state.pageOffset = 50;
    t.state.totalTasks = 60;
    await t.loadTasks();
    ok(t.state.tasks.length === 10, '第 2 页加载到 10 条', 'got ' + t.state.tasks.length);

    calls = [];
    await t.loadTasks();
    ok(calls.length === 1 && calls[0].includes('offset=50'),
       '重新加载仍请求 offset=50', calls[0]);
    ok(t.state.tasks.length === 10, '列表仍是 10 条未被撑成全量', 'got ' + t.state.tasks.length);
  }

  console.log('== P0-1b: 筛选不被冲掉 ==');
  {
    H.setFetch(makeFetch(u => u.includes('state=failed')
      ? { tasks: [{chapterId:'ch-f1', state:'failed', comicId:'c1', chapterTitle:'x', doneImgCount:1, totalImgCount:2}], total: 1 }
      : { tasks: page1Tasks(), total: 60 }));
    const t = H.load();
    t.state.filterState = 'failed';
    await t.loadTasks();
    ok(t.state.tasks.length === 1 && t.state.tasks[0].chapterId === 'ch-f1',
       '筛选生效只返回 failed 的 1 条');

    calls = [];
    await t.loadTasks();
    ok(calls[0].includes('state=failed'), '重新加载仍带 state=failed', calls[0]);
    ok(t.state.tasks.length === 1, '筛选未被冲成全量', 'got ' + t.state.tasks.length);
  }

  console.log('== P0-2: 加载失败区分于空态 ==');
  {
    H.setFetch(async () => ({ ok: false, status: 500, statusText: 'Internal Server Error',
      text: async () => JSON.stringify({ error: '数据库连接失败' }) }));
    const t = H.load();
    await t.loadTasks();
    const html = H.written['task-rows'] || '';
    ok(html.includes('任务列表加载失败'), '显示加载失败而非空态', html.slice(0, 100));
    ok(html.includes('数据库连接失败'), '带上了后端错误原文');
    ok(!html.includes('没有匹配的任务'), '没有误显示空态文案');
    ok(t.state.tasks.length === 0, 'tasks 已清空');
  }

  console.log('== P0-2b: 真的没有任务是空态不是错误态 ==');
  {
    H.setFetch(makeFetch(() => ({ tasks: [], total: 0 })));
    const t = H.load();
    await t.loadTasks();
    const html = H.written['task-rows'] || '';
    ok(!html.includes('任务列表加载失败'), '空结果不报错');
    ok(t.state.tasks.length === 0 && t.state.totalTasks === 0, 'total 归零');
  }

  console.log('== 竞态: 慢响应不覆盖新数据 loadSeq ==');
  {
    let resolveSlow = null, n = 0;
    H.setFetch(async () => {
      n++;
      if (n === 1) {
        return new Promise(r => { resolveSlow = () => r({ ok:true, status:200, statusText:'OK',
          text: async () => JSON.stringify({ tasks: [{chapterId:'OLD', state:'pending', comicId:'c1', chapterTitle:'old', doneImgCount:0, totalImgCount:1}], total: 1 }) }); });
      }
      return { ok:true, status:200, statusText:'OK',
        text: async () => JSON.stringify({ tasks: [
          {chapterId:'NEW1', state:'pending', comicId:'c1', chapterTitle:'n1', doneImgCount:0, totalImgCount:1},
          {chapterId:'NEW2', state:'pending', comicId:'c1', chapterTitle:'n2', doneImgCount:0, totalImgCount:1}], total: 2 }) };
    });
    const t = H.load();
    const p1 = t.loadTasks();
    await t.loadTasks();
    ok(t.state.tasks.length === 2 && t.state.tasks[0].chapterId === 'NEW1',
       '快响应已渲染 2 条新数据', JSON.stringify(t.state.tasks.map(x=>x.chapterId)));
    resolveSlow();
    await p1;
    ok(t.state.tasks[0].chapterId === 'NEW1',
       '慢响应回来后被丢弃', JSON.stringify(t.state.tasks.map(x=>x.chapterId)));
    ok(t.state.tasks.length === 2, '仍是 2 条');
  }

  console.log('== applyTaskEvent: 同页任务就地更新 ==');
  {
    H.setFetch(makeFetch(() => ({ tasks: page1Tasks(), total: 60 })));
    const t = H.load();
    await t.loadTasks();
    const target = t.state.tasks[0].chapterId;
    t.applyTaskEvent({ event: 'Update', data: { chapterId: target, state: 'downloading',
      downloadedImgCount: 5, totalImgCount: 10, retryCount: 1 } });
    const after = t.state.tasks.find(x => x.chapterId === target);
    ok(after.state === 'downloading', '状态就地更新');
    ok(after.doneImgCount === 5, '进度更新为 5');
    ok(after.progress === 50, '百分比算对 50', 'got ' + after.progress);
  }

  console.log('');
  console.log('== 结果: ' + pass + ' passed, ' + fail + ' failed ==');
  process.exit(fail > 0 ? 1 : 0);
})();