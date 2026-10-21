import fs from 'node:fs';
import path from 'node:path';
import {root} from './run.mjs';

const required = ['logo.png', 'soundtrack.wav', ...['main', 'block-editor', 'assistant', 'media-crop'].map(n => `shots/${n}.png`), ...Array.from({length: 15}, (_, n) => `keys/key-${String(n).padStart(2, '0')}.png`), ...Array.from({length: 12}, (_, n) => `keys/key-09-f${String(n).padStart(2, '0')}.png`)];
const missing = required.filter(n => !fs.existsSync(path.join(root, 'public', n)));
if (missing.length) {
  console.error('Ressources absentes (captures NovyDeck indispensables):\n' + missing.map(n => `  public/${n}`).join('\n'));
  process.exit(1);
}
console.log(`Ressources validées : ${required.length} fichiers présents.`);
