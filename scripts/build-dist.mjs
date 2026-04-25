#!/usr/bin/env node
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Post-build artifact organizer.
// 1. Locates the Tauri release binary
// 2. Renames to `Hando-{platform}-{arch}-v{version}.{ext}`
// 3. Zips for portable distribution
//
// Usage: node scripts/build-dist.mjs

import { readFile, copyFile, mkdir, stat } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { platform, arch } from 'node:os';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

async function main() {
  const cargoToml = await readFile(join(ROOT, 'src-tauri/Cargo.toml'), 'utf8');
  const versionMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
  if (!versionMatch) throw new Error('Could not parse version from Cargo.toml');
  const version = versionMatch[1];

  const plat = platform(); // 'win32', 'darwin', 'linux'
  const a = arch();        // 'x64', 'arm64'

  const platTag = plat === 'win32' ? 'win' : plat === 'darwin' ? 'mac' : plat;
  const ext = plat === 'win32' ? '.exe' : '.app';
  const srcBinary = plat === 'win32'
    ? join(ROOT, 'src-tauri/target/release/hando.exe')
    : join(ROOT, 'src-tauri/target/release/bundle/macos/Hando.app');

  await stat(srcBinary).catch(() => {
    throw new Error(`Binary not found at ${srcBinary} — run \`npm run tauri build\` first`);
  });

  const distDir = join(ROOT, 'dist-final');
  await mkdir(distDir, { recursive: true });

  const niceName = `Hando-${platTag}-${a}-v${version}${ext}`;
  const dstPath = join(distDir, niceName);
  await copyFile(srcBinary, dstPath);
  console.log(`Renamed: ${dstPath}`);

  // Zip — use system 'zip' on macOS/linux, PowerShell Compress-Archive on Windows.
  const zipName = `${niceName}.zip`;
  const zipPath = join(distDir, zipName);
  await zipFile(dstPath, zipPath);
  console.log(`Zipped:  ${zipPath}`);
}

function zipFile(src, dst) {
  return new Promise((resolve, reject) => {
    const isWin = platform() === 'win32';
    const cmd = isWin ? 'powershell' : 'zip';
    const args = isWin
      ? ['-NoProfile', '-Command', `Compress-Archive -Path '${src}' -DestinationPath '${dst}' -Force`]
      : ['-r', '-j', dst, src];
    const child = spawn(cmd, args, { stdio: 'inherit' });
    child.on('exit', (code) => code === 0 ? resolve() : reject(new Error(`${cmd} exited ${code}`)));
    child.on('error', reject);
  });
}

main().catch((err) => { console.error(err); process.exit(1); });
