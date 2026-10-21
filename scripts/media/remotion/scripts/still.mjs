import fs from 'node:fs';
import path from 'node:path';
import {run, root} from './run.mjs';
const frame = Number(process.argv[2] ?? 90);
if (!Number.isInteger(frame) || frame < 0 || frame >= 1290) {console.error('Frame entre 0 et 1289 attendu.'); process.exit(1);}
const file = process.argv[3] ?? `out/still-${frame}.png`;
fs.mkdirSync(path.dirname(path.resolve(root, file)), {recursive: true});
run(['still', 'src/index.ts', 'NovyDeckDemo', file, `--frame=${frame}`, '--gl=angle', ...process.argv.slice(4)]);
