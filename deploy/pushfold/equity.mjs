// Gera a tabela de equity pré-flop 169 × 169 (classe de mão contra classe de mão, all-in até o river)
// usada pelo Push/fold. Roda no GitHub Actions (com cache); o avaliador de mãos é o mesmo da página.
// Uso: node deploy/pushfold/equity.mjs <saida.bin> [amostras por par]
// Saída: Uint16 little-endian, equity × 65535, índice linha * 169 + coluna, na ordem da grade 13 × 13
// (linha/coluna 0 = A; acima da diagonal suited, abaixo offsuit).
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { availableParallelism } from 'node:os';
import { Worker, isMainThread, parentPort, workerData } from 'node:worker_threads';

const html = readFileSync(new URL('../../static/index.html', import.meta.url), 'utf8');
const math = html.slice(html.indexOf('// ---------- matemática de poker'), html.indexOf('// ---------- treinar'));
const { handValue } = new Function(math + '; return { handValue };')();

// Combos de cada classe com cartas rank * 4 + naipe (rank 0 = 2 … 12 = A), como na página.
function classCombos(k) {
  const r = Math.floor(k / 13), c = k % 13, hi = 12 - Math.min(r, c), lo = 12 - Math.max(r, c), out = [];
  for (let s1 = 0; s1 < 4; s1++) for (let s2 = 0; s2 < 4; s2++) {
    if (r === c ? s2 <= s1 : r < c ? s1 !== s2 : s1 === s2) continue;
    out.push([hi * 4 + s1, lo * 4 + s2]);
  }
  return out;
}

function equityRows(rows, samples) {
  const combos = Array.from({ length: 169 }, (_, k) => classCombos(k));
  const out = {}, deck = new Array(52), a7 = new Array(7), b7 = new Array(7);
  for (const i of rows) {
    const row = new Float64Array(169);
    for (let j = i + 1; j < 169; j++) {
      let score = 0, n = 0;
      while (n < samples) {
        const a = combos[i][Math.floor(Math.random() * combos[i].length)];
        const b = combos[j][Math.floor(Math.random() * combos[j].length)];
        if (a[0] === b[0] || a[0] === b[1] || a[1] === b[0] || a[1] === b[1]) continue;
        let m = 0;
        for (let c = 0; c < 52; c++) if (c !== a[0] && c !== a[1] && c !== b[0] && c !== b[1]) deck[m++] = c;
        for (let x = 0; x < 5; x++) { const y = x + Math.floor(Math.random() * (m - x)); [deck[x], deck[y]] = [deck[y], deck[x]]; }
        a7[0] = a[0]; a7[1] = a[1]; b7[0] = b[0]; b7[1] = b[1];
        for (let x = 0; x < 5; x++) a7[x + 2] = b7[x + 2] = deck[x];
        const va = handValue(a7), vb = handValue(b7);
        score += va > vb ? 1 : va === vb ? 0.5 : 0;
        n++;
      }
      row[j] = score / samples;
    }
    out[i] = Array.from(row);
  }
  return out;
}

if (!isMainThread) {
  parentPort.postMessage(equityRows(workerData.rows, workerData.samples));
} else {
  const [file, samplesArg] = process.argv.slice(2);
  if (!file) { console.error('uso: node deploy/pushfold/equity.mjs <saida.bin> [amostras]'); process.exit(1); }
  const samples = Number(samplesArg) || 6000;
  const threads = Math.max(1, availableParallelism());
  const started = Date.now();
  // Linhas do início têm mais pares; distribui intercalando para equilibrar as threads.
  const parts = Array.from({ length: threads }, (_, t) => Array.from({ length: 169 }, (_, i) => i).filter(i => i % threads === t));
  const results = await Promise.all(parts.map(rows => new Promise((resolve, reject) => {
    const w = new Worker(new URL(import.meta.url), { workerData: { rows, samples } });
    w.on('message', resolve); w.on('error', reject);
  })));
  const E = new Float64Array(169 * 169);
  for (const res of results) for (const [key, row] of Object.entries(res)) for (let i = Number(key), j = i + 1; j < 169; j++) {
    E[i * 169 + j] = row[j];
    E[j * 169 + i] = 1 - row[j];
  }
  for (let i = 0; i < 169; i++) E[i * 169 + i] = 0.5;

  // Conferência com valores conhecidos (tolerância folgada para o Monte Carlo).
  const idx = (h) => {
    const R = 'AKQJT98765432', r1 = R.indexOf(h[0]), r2 = R.indexOf(h[1]);
    return r1 === r2 ? r1 * 13 + r1 : h[2] === 's' ? r1 * 13 + r2 : r2 * 13 + r1;
  };
  const check = (a, b, want) => {
    const got = E[idx(a) * 169 + idx(b)];
    if (Math.abs(got - want) > 0.02) throw new Error(`${a} vs ${b}: ${got.toFixed(3)}, esperado ~${want}`);
  };
  check('AA', 'KK', 0.82); check('AKo', '22', 0.47); check('AKs', 'QQ', 0.46); check('72o', 'AA', 0.12); check('JTs', '22', 0.53);

  const out = new Uint16Array(169 * 169);
  E.forEach((x, k) => out[k] = Math.round(x * 65535));
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, Buffer.from(out.buffer));
  console.log(`equity pré-flop: ${samples} amostras por par, ${threads} threads, ${((Date.now() - started) / 1000).toFixed(0)} s`);
}
