#!/usr/bin/env bash
# 跑 src-server 单文件控制台的全部前端行为测试。
#
#   src-server/tests/frontend/run-all.sh
#
# 测的是 src-server/static/index.html —— 后端 include_str! 嵌进二进制的那个控制台。
# 它和仓库根的 Vue 应用（src/）是两套独立前端，不要混淆。
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rc=0

for t in test-behavior.cjs test-ws.cjs; do
  echo "════════ $t ════════"
  node "$DIR/$t" || rc=1
  echo
done

if [ "$rc" -eq 0 ]; then
  echo "✅ 全部通过"
else
  echo "❌ 有失败用例"
fi
exit "$rc"