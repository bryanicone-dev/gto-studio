// Escolhe N flops representativos (padrão 184) dos 1.755 diferentes, para a lista "wide" de spots.json.
// Cada flop pesa pelo número de flops reais equivalentes; a cota de cada grupo (naipes × par) é proporcional
// a esse peso e, dentro do grupo, os flops são tirados a intervalos iguais de peso, em ordem de ranks.
// Os flops de "boards" contam na cota e não se repetem. Uso: node deploy/library/flops.mjs [N] > flops.json
import { readFileSync } from 'node:fs';

const N = Number(process.argv[2] || 184);
const spec = JSON.parse(readFileSync(new URL('./spots.json', import.meta.url), 'utf8'));
const RANKS = '23456789TJQKA', SUITS = 'shdc';
// Mesma chave do site e de extra.mjs: ranks em ordem e naipes renomeados na ordem em que aparecem.
function flopKey(board) {
  const cards = board.match(/../g).map(c => ({ r: RANKS.indexOf(c[0]), s: c[1] }));
  return [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]].map(p => p.map(i => cards[i]))
    .filter(p => p[0].r >= p[1].r && p[1].r >= p[2].r)
    .map(p => { const m = {}; return p.map(c => RANKS[c.r] + (m[c.s] ??= 'xyz'[Object.keys(m).length])).join(''); })
    .sort()[0];
}

const deck = [...RANKS].flatMap(r => [...SUITS].map(s => r + s));
const weight = new Map();
for (let a = 0; a < 52; a++) for (let b = a + 1; b < 52; b++) for (let c = b + 1; c < 52; c++) {
  const k = flopKey(deck[a] + deck[b] + deck[c]);
  weight.set(k, (weight.get(k) || 0) + 1);
}
const group = (k) => k.replace(/[2-9TJQKA]/g, '') + (new Set(k.match(/[2-9TJQKA]/g)).size < 3 ? 'p' : '');
const order = (k) => k.match(/[2-9TJQKA]/g).map(r => String(RANKS.indexOf(r)).padStart(2, '0')).join('');
const fixed = new Set(spec.boards.map(flopKey));
const total = [...weight.values()].reduce((s, w) => s + w, 0);

const out = [];
for (const g of [...new Set([...weight.keys()].map(group))].sort()) {
  const keys = [...weight.keys()].filter(k => group(k) === g).sort((a, b) => order(b).localeCompare(order(a)));
  const w = keys.reduce((s, k) => s + weight.get(k), 0);
  const free = keys.filter(k => !fixed.has(k)), fw = free.reduce((s, k) => s + weight.get(k), 0);
  const quota = Math.max(1, Math.round(N * w / total)) - (keys.length - free.length);
  let acc = 0, next = 0;
  for (const k of free) {
    acc += weight.get(k);
    if (next < quota && acc >= (next + 0.5) * fw / quota) { out.push(k); next++; }
  }
}
// x, y, z viram naipes de verdade (s, h, d).
console.log(JSON.stringify(out.map(k => k.replace(/[xyz]/g, s => 'shd'['xyz'.indexOf(s)]))));
console.error(`${out.length} flops novos + ${fixed.size} de "boards" = ${out.length + fixed.size}`);
