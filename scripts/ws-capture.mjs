// 一次性抓帧脚本：连后端 WS，把每一帧原样打出来，不做过滤/解析/美化。
// 用途：核对后端发出的 payload 字段名、状态字面量、事件时序。
// 用法：node scripts/ws-capture.mjs [秒数]
//
// 边界（重要）：本脚本直连 :8080，绕过 vite 代理，也不经过前端。
// 它只能证明「后端发了什么」，不能证明「代理转发了」或「前端渲染了」。

const seconds = Number(process.argv[2] ?? 60)
const url = 'ws://127.0.0.1:8080/api/ws'

const t0 = Date.now()
const ts = () => `+${((Date.now() - t0) / 1000).toFixed(3)}s`

let frames = 0

const ws = new WebSocket(url)

ws.addEventListener('open', () => {
  console.log(`[${ts()}] OPEN ${url}`)
})

ws.addEventListener('message', (ev) => {
  frames += 1
  let topic = '(unparsed)'
  try {
    const parsed = JSON.parse(ev.data)
    topic = parsed?.topic ?? parsed?.type ?? '(no topic field)'
  } catch (err) {
    topic = `(json parse failed: ${err.message})`
  }

  console.log(`--- FRAME #${frames} [${ts()}] topic=${topic} ---`)
  // 逐字原样打印，不 JSON.stringify 美化
  console.log(ev.data)
  console.log(`--- END FRAME #${frames} ---`)
})

ws.addEventListener('error', (ev) => {
  console.log(`[${ts()}] ERROR ${ev.message ?? ev.type ?? 'unknown'}`)
})

ws.addEventListener('close', (ev) => {
  console.log(`[${ts()}] CLOSE code=${ev.code} reason=${ev.reason}`)
})

setTimeout(() => {
  console.log(`[${ts()}] TIMEOUT after ${seconds}s, total frames = ${frames}`)
  ws.close()
  process.exit(0)
}, seconds * 1000)