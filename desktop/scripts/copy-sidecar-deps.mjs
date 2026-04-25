// Copies sharp + deps into desktop/src-tauri/sidecar-deps/ so Tauri
// can reference them with simple relative paths (no ../../ glob issues).
// Run from repo root: node desktop/scripts/copy-sidecar-deps.mjs
import { cpSync, mkdirSync, rmSync, existsSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const destBase = resolve(repoRoot, 'desktop/src-tauri/sidecar-deps');

rmSync(destBase, { recursive: true, force: true });

const deps = [
  'node_modules/sharp',
  'node_modules/@img/colour',
  'node_modules/@img/sharp-win32-x64',
  'node_modules/detect-libc',
  'node_modules/semver',
];

for (const dep of deps) {
  const src = resolve(repoRoot, dep);
  const dest = resolve(destBase, dep);
  if (!existsSync(src)) { console.warn(`SKIP (not found): ${dep}`); continue; }
  mkdirSync(dest, { recursive: true });
  cpSync(src, dest, { recursive: true });
  console.log(`Copied ${dep}`);
}

console.log('sidecar-deps ready at desktop/src-tauri/sidecar-deps/');
