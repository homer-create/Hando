// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { readDir, stat } from '@tauri-apps/plugin-fs';

const SUPPORTED = new Set(['.jpg', '.jpeg', '.png', '.webp']);

export async function expandPaths(paths: string[]): Promise<{ files: string[]; skipped: number }> {
  const files: string[] = [];
  let skipped = 0;
  for (const p of paths) {
    const s = await stat(p);
    if (s.isDirectory) {
      for (const entry of await readDir(p)) {
        if (!entry.name) continue;
        const full = `${p}/${entry.name}`;
        const res = await expandPaths([full]);
        files.push(...res.files);
        skipped += res.skipped;
      }
    } else {
      const idx = p.lastIndexOf('.');
      const ext = idx >= 0 ? p.slice(idx).toLowerCase() : '';
      if (SUPPORTED.has(ext)) files.push(p);
      else skipped++;
    }
  }
  return { files, skipped };
}
