// Web Worker of the browser build: runs the WebAssembly solver off the UI thread.
// Messages in: {id, cmd: 'solve' | 'cancel' | 'node' | 'load' | 'resolve', body}. Replies: {id, result}.
// While solving it also posts {type: 'progress', ...status} after every batch.
import init, { prepare, step, finish, node, load, resolve } from './pkg/gto_studio_web.js';

const ready = init();
let cancelled = false;

const reply = (id, result) => postMessage({ id, result });

self.onmessage = async ({ data: { id, cmd, body } }) => {
  await ready;
  if (cmd === 'cancel') { cancelled = true; return reply(id, {}); }
  if (cmd === 'node') return reply(id, JSON.parse(node(JSON.stringify(body.history))));
  if (cmd === 'load') return reply(id, JSON.parse(load(new Uint8Array(body.bytes))));
  if (cmd === 'resolve') {
    const { prefix, card, limit_gb, max_iterations, target_pct } = body;
    return reply(id, JSON.parse(resolve(JSON.stringify(prefix), card, limit_gb, max_iterations, target_pct)));
  }
  if (cmd === 'solve') {
    const result = JSON.parse(prepare(JSON.stringify(body), body.limit_gb));
    reply(id, result);
    if (!result.error) run();
  }
};

async function run() {
  cancelled = false;
  for (;;) {
    const s = JSON.parse(step(1));
    postMessage({ type: 'progress', state: 'solving', ...s });
    if (s.done || cancelled) break;
    await new Promise((r) => setTimeout(r, 0)); // let "stop" messages in
  }
  finish();
  postMessage({ type: 'progress', state: 'done', cancelled });
}
