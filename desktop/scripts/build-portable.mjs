// Assembles Hando-portable/ from the last release build.
// Run from desktop/: node scripts/build-portable.mjs
import { cpSync, mkdirSync, rmSync, existsSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { execSync } from 'child_process';

const desktopDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot   = resolve(desktopDir, '..');
const portableDir = resolve(desktopDir, 'Hando-portable');
const releaseDir  = resolve(desktopDir, 'src-tauri/target/release');

// 1. Always run cargo build (incremental — fast if nothing changed,
//    but guarantees the exe reflects the latest source)
console.log('Running cargo build --release...');
execSync('cargo build --release', {
  cwd: resolve(desktopDir, 'src-tauri'),
  stdio: 'inherit',
});
const exePath = resolve(releaseDir, 'hando.exe');
if (!existsSync(exePath)) {
  console.error('hando.exe still not found after build');
  process.exit(1);
}

// 2. Ensure sidecar-deps is populated
const depsDir = resolve(desktopDir, 'src-tauri/sidecar-deps/node_modules');
if (!existsSync(depsDir)) {
  console.log('sidecar-deps not found — running copy-sidecar-deps.mjs...');
  execSync('node scripts/copy-sidecar-deps.mjs', { cwd: desktopDir, stdio: 'inherit' });
}

// 3. Clean and create portable dir
rmSync(portableDir, { recursive: true, force: true });
mkdirSync(portableDir, { recursive: true });

// 4. Main exe
cpSync(exePath, resolve(portableDir, 'hando.exe'));
console.log('✓ hando.exe');

// 5. Node runtime
const nodeBin = resolve(desktopDir, 'src-tauri/binaries/node-x86_64-pc-windows-msvc.exe');
cpSync(nodeBin, resolve(portableDir, 'node.exe'));
console.log('✓ node.exe');

// 6. Sidecar JS scripts
mkdirSync(resolve(portableDir, 'src'));
for (const f of ['sidecar.js', 'encoder.js', 'config.js']) {
  cpSync(resolve(repoRoot, `src/${f}`), resolve(portableDir, `src/${f}`));
}
console.log('✓ src/ (sidecar.js, encoder.js, config.js)');

// 7. Sharp + deps
cpSync(depsDir, resolve(portableDir, 'node_modules'), { recursive: true });
console.log('✓ node_modules/ (sharp + deps)');

// 8. Zip it
const zipPath = resolve(desktopDir, 'Hando-portable.zip');
execSync(`powershell -Command "Compress-Archive -Path '${portableDir}\\*' -DestinationPath '${zipPath}' -Force"`, { stdio: 'inherit' });
console.log(`✓ Hando-portable.zip`);

console.log(`\nDone!`);
console.log(`  Folder : ${portableDir}`);
console.log(`  Zip    : ${zipPath}`);
