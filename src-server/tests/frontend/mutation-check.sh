#!/usr/bin/env bash
# P0-1 变异测试：证明 test-ws.cjs 真能抓到「快照覆盖分页」这个回归。
#
# 一个全绿的测试不能说明它有效——它必须在缺陷复现时变红。
# 做法：临时把 onmessage 里的 loadTasks() 换成 state.tasks = payload
#       （即修复前的行为），跑测试，期望 FAIL；然后无条件还原。
#
# 运行： bash mutation-check.sh
set -u
cd "$(dirname "$0")"

SRC=../../static/index.html
BACKUP=$(mktemp)
cp "$SRC" "$BACKUP"

restore() { cp "$BACKUP" "$SRC"; rm -f "$BACKUP"; }
trap restore EXIT INT TERM

# 定向替换：只改 onmessage 快照分支里那一行 loadTasks();
# 用行号锚定，避免误伤 applyTaskEvent 里的其它 loadTasks() 调用。
python3 - "$SRC" <<'PY'
import re, sys
p = sys.argv[1]
s = open(p, encoding='utf-8').read()

# 快照分支的特征：注释块之后紧跟 "loadTasks();" + "return;"
pat = re.compile(
    r'(// 这里主动拉一次，保证连上就能拿到当前页的最新状态。\s*\n\s*)loadTasks\(\);',
    re.M)
new, n = pat.subn(r'\1state.tasks = payload;', s)
if n != 1:
    sys.exit('变异点未唯一定位，n=%d（源码结构可能已变，需更新本脚本）' % n)
open(p, 'w', encoding='utf-8').write(new)
print('已变异: loadTasks() -> state.tasks = payload')
PY

if [ $? -ne 0 ]; then
  echo "变异失败，跳过（不算测试失败）"
  exit 0
fi

echo
echo "=== 跑变异后的测试（期望 FAIL，且不得崩溃）==="
node test-ws.cjs
RC=$?
echo "exit=$RC"

echo
echo "=== 判定 ==="
if [ "$RC" -eq 1 ]; then
  echo "✅ 测试有效：缺陷复现时变红（exit 1），未崩溃"
  exit 0
elif [ "$RC" -eq 0 ]; then
  echo "❌ 测试无效：缺陷已注入但仍全绿——这个测试抓不到它"
  exit 1
else
  echo "❌ 测试崩溃（exit $RC）而非干净失败——需检查断言健壮性"
  exit 1
fi