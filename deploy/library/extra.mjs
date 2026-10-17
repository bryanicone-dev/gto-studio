// Flops pedidos pela análise de mãos do site (Supabase) para uma situação, sem repetir flops equivalentes
// por naipes nem os que já estão em spots.json. Uso: node deploy/library/extra.mjs "<preset>" > extra.json
// Lê SUPABASE_URL e SUPABASE_KEY (chave publishable, a mesma do site). Sem acesso, devolve [].
import { readFileSync } from 'node:fs';

const preset = process.argv[2];
const spec = JSON.parse(readFileSync(new URL('./spots.json', import.meta.url), 'utf8'));
const RANKS = '23456789TJQKA';
// Mesma chave do site: ranks em ordem e naipes renomeados na ordem em que aparecem.
function flopKey(board) {
  const cards = board.match(/../g).map(c => ({ r: RANKS.indexOf(c[0]), s: c[1] }));
  return [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]].map(p => p.map(i => cards[i]))
    .filter(p => p[0].r >= p[1].r && p[1].r >= p[2].r)
    .map(p => { const m = {}; return p.map(c => RANKS[c.r] + (m[c.s] ??= 'xyz'[Object.keys(m).length])).join(''); })
    .sort()[0];
}
let rows = [];
try {
  const { SUPABASE_URL: url, SUPABASE_KEY: key } = process.env;
  const r = await fetch(url + '/rest/v1/rpc/list_spot_requests', { method: 'POST', headers: { apikey: key, 'Content-Type': 'application/json' }, body: '{}' });
  if (r.ok) rows = await r.json();
} catch {}
const seen = new Set(spec.boards.map(flopKey));
const out = [];
for (const { preset: p, board } of rows) {
  if (p !== preset || !/^([2-9TJQKA][cdhs]){3}$/.test(board) || new Set(board.match(/../g)).size !== 3) continue;
  const k = flopKey(board);
  if (seen.has(k)) continue;
  seen.add(k);
  out.push(board);
}
console.log(JSON.stringify(out));
