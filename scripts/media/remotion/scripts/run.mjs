import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import fs from 'node:fs';
import path from 'node:path';

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export function run(args) {
  const pkgDir = path.join(root, 'node_modules', '@remotion', 'cli');
  const pkg = JSON.parse(fs.readFileSync(path.join(pkgDir, 'package.json'), 'utf8'));
  const cli = path.join(pkgDir, typeof pkg.bin === 'string' ? pkg.bin : pkg.bin.remotion);
  const r = spawnSync(process.execPath, [cli, ...args], {cwd: root, stdio: 'inherit', shell: false});
  if (r.error) {console.error(r.error.message); process.exit(1);}
  process.exit(r.status ?? 1);
}
