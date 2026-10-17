// Junta as bibliotecas geradas por preset (uma pasta por máquina do CI) numa pasta só,
// com um index.json na ordem de deploy/library/spots.json.
// Uso: node deploy/library/merge.mjs <pasta-com-as-partes> <pasta-de-saida>
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [parts, out] = process.argv.slice(2);
const spec = JSON.parse(readFileSync('deploy/library/spots.json', 'utf8'));
// Flops fixos na ordem de spots.json (boards e depois wide); os pedidos pela análise de mãos (extra) vêm depois, em ordem alfabética.
// Uma situação pode vir de várias máquinas (wide), cada uma com uma cópia dos spots antigos: cada id entra uma vez só.
const fixed = [...spec.boards, ...(spec.wide?.boards || [])];
const isFixed = (s) => spec.boards.includes(s.board) || (s.preset in (spec.wide?.presets || {}) && spec.wide.boards.includes(s.board));
const rank = (s) => spec.presets.indexOf(s.preset) * 10000 + (isFixed(s) ? fixed.indexOf(s.board) : 5000);

mkdirSync(out, { recursive: true });
const index = [], seen = new Set();
for (const dir of existsSync(parts) ? readdirSync(parts) : []) {
  const part = join(parts, dir);
  if (!existsSync(join(part, 'index.json'))) continue;
  for (const s of JSON.parse(readFileSync(join(part, 'index.json'), 'utf8'))) {
    if (spec.presets.indexOf(s.preset) < 0 || !(s.extra || isFixed(s)) || seen.has(s.id) || !existsSync(join(part, s.file))) continue;
    seen.add(s.id);
    copyFileSync(join(part, s.file), join(out, s.file));
    index.push(s);
  }
}
index.sort((a, b) => rank(a) - rank(b) || a.board.localeCompare(b.board));
writeFileSync(join(out, 'index.json'), JSON.stringify(index, null, 2));
console.log(`${index.length} spots na biblioteca`);
