import fs from 'node:fs';
import path from 'node:path';
import {run, root} from './run.mjs';
const names = ['main', 'block-editor', 'assistant', 'media-crop'];
const missing = names.filter(n => !fs.existsSync(path.join(root, 'public', 'shots', n + '.png')));
if (missing.length) {console.error('Captures NovyDeck requises : ' + missing.join(', ')); process.exit(1);}
const file = process.argv[2] ?? 'out/novydeck-v0.8.11.mp4';
fs.mkdirSync(path.dirname(path.resolve(root, file)), {recursive: true});
run(['render', 'src/index.ts', 'NovyDeckDemo', file, '--gl=angle', '--codec=h264', '--crf=18', '--concurrency=2', ...process.argv.slice(3)]);
