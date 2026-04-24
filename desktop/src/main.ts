// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { mountToolbar } from './ui/toolbar';
import { mountDropzone } from './ui/dropzone';
import { mountFileList } from './ui/file-list';
import { mountStatusBar } from './ui/statusbar';
import { store, anyWorking } from './state';
import { basename } from './util/path';
import { openSettingsPanel, loadSettings, getSettings } from './ui/settings';
import { compress, toOpts, onFileDone, onFileError, onFileSkipped, undoLastBatch } from './ipc';
import { expandPaths } from './fs';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

function extOf(p: string): string {
  const idx = p.lastIndexOf('.');
  return idx >= 0 ? p.slice(idx).toLowerCase() : '';
}

async function main() {
  await loadSettings();

  // Task 14: Subscribe to sidecar events BEFORE mounting UI
  onFileDone((p) => {
    const row = store.snapshotById(p.id);
    if (!row) return;
    store.update(row.path, { status: 'done', srcBytes: p.srcBytes, outBytes: p.outBytes });
  });
  onFileError((p) => {
    const row = store.snapshotById(p.id);
    if (!row) return;
    store.update(row.path, { status: 'error', errorMsg: p.msg });
  });
  onFileSkipped((p) => {
    const row = store.snapshotById(p.id);
    if (!row) return;
    store.update(row.path, { status: 'skipped-no-gain', srcBytes: p.srcBytes });
  });

  const toolbar = mountToolbar(document.getElementById('toolbar')!, {
    onSettings: () => openSettingsPanel(),
    onUndo: async () => {
      const r = await undoLastBatch();
      console.log(`undone: ${r.restored}/${r.attempted}`);
      toolbar.setUndoEnabled(false);
      store.clear();
    },
  });

  // Enable Undo when at least one row is done and none are working
  store.subscribe((rows) => {
    const hasTerminal = rows.some((r) => r.status === 'done');
    const anyWorking = rows.some((r) => r.status === 'working' || r.status === 'pending');
    toolbar.setUndoEnabled(hasTerminal && !anyWorking);
  });

  await mountDropzone(document.getElementById('dropzone')!, async (paths) => {
    const { files: expandedPaths, skipped } = await expandPaths(paths);
    if (skipped > 0) console.log(`Skipped ${skipped} unsupported`);
    const files = expandedPaths.map((p) => ({
      id: crypto.randomUUID(),
      path: p,
      ext: extOf(p),
      name: basename(p),
    }));
    for (const f of files) store.upsert({ id: f.id, path: f.path, name: f.name, status: 'working' });
    try {
      await compress({
        batchId: crypto.randomUUID(),
        files: files.map(({ id, path, ext }) => ({ id, path, ext })),
        opts: toOpts(getSettings()),
        moveOriginalsToTrash: getSettings().moveOriginalsToTrash,
      });
    } catch (err) {
      console.error('compress failed:', err);
      for (const f of files) store.update(f.path, { status: 'error', errorMsg: String(err) });
    }
  });

  mountFileList(document.getElementById('list')!);
  mountStatusBar(document.getElementById('statusbar')!);

  listen('close-requested', async () => {
    if (anyWorking()) {
      const count = store.snapshot().filter((r) => r.status === 'working' || r.status === 'pending').length;
      if (!confirm(`${count} files still processing. Quit anyway?`)) return;
    }
    await invoke('confirm_close');
  });

  listen('sidecar-crashed', () => {
    const snap = store.snapshot();
    for (const r of snap) {
      if (r.status === 'pending' || r.status === 'working') {
        store.update(r.path, { status: 'error', errorMsg: 'Engine crashed' });
      }
    }
    alert('Image engine crashed. It will restart on the next drop.');
  });
}
main();
