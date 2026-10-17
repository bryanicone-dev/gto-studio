// Junta as bibliotecas geradas por preset (uma pasta por máquina do CI) numa pasta só,
// com um index.json na ordem de deploy/library/spots.json.
// Uso: node deploy/library/merge.mjs <pasta-com-as-partes> <pasta-de-saida>
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [parts, out] = process.argv.slice(2);
const spec = JSON.parse(readFileSync('deploy/library/spots.json', 'utf8'));
// Flops fixos na ordem de spots.json; os pedidos pela análise de mãos (extra) vêm depois, em ordem alfabética.
const rank = (s) => spec.presets.indexOf(s.preset) * 1000 + (spec.boards.includes(s.board) ? spec.boards.indexOf(s.board) : 500);

mkdirSync(out, { recursive: true });
const index = [];
for (const dir of existsSync(parts) ? readdirSync(parts) : []) {
  const part = join(parts, dir);
  if (!existsSync(join(part, 'index.json'))) continue;
  for (const s of JSON.parse(readFileSync(join(part, 'index.json'), 'utf8'))) {
    if (spec.presets.indexOf(s.preset) < 0 || !(s.extra || spec.boards.includes(s.board)) || !existsSync(join(part, s.file))) continue;
    copyFileSync(join(part, s.file), join(out, s.file));
    index.push(s);
  }
}
index.sort((a, b) => rank(a) - rank(b) || a.board.localeCompare(b.board));
writeFileSync(join(out, 'index.json'), JSON.stringify(index, null, 2));
console.log(`${index.length} spots na biblioteca`);
