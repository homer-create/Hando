// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { mountToolbar } from './ui/toolbar';
import { mountDropzone } from './ui/dropzone';
import { mountFileList } from './ui/file-list';
import { mountStatusBar } from './ui/statusbar';
import { store, anyWorking } from './state';
import { basename } from './util/path';
import { openSettingsPanel, loadSettings, getSettings } from './ui/settings';
import { compress, toOpts, onFileDone, onFileError, onFileSkipped, onBatchDone, undoLastBatch } from './ipc';
import { expandPaths } from './fs';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import * as i18n from './i18n';
import { initTheme } from './ui/theme';

function extOf(p: string): string {
  const idx = p.lastIndexOf('.');
  return idx >= 0 ? p.slice(idx).toLowerCase() : '';
}

async function main() {
  await loadSettings();
  i18n.init(getSettings().language);
  initTheme();

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

  // Backend signals end-of-batch authoritatively
  onBatchDone(() => {
    const rows = store.snapshot();
    const hasTerminal = rows.some((r) => r.status === 'done');
    toolbar.setUndoEnabled(hasTerminal);
  });

  const dropzone = await mountDropzone(document.getElementById('dropzone')!, async (paths) => {
    const { files: expandedPaths, skipped } = await expandPaths(paths);
    if (skipped > 0) console.log(`Skipped ${skipped} unsupported`);
    const files = expandedPaths.map((p) => ({
      id: crypto.randomUUID(),
      path: p,
      ext: extOf(p),
      name: basename(p),
    }));
    // Clear previous results when starting a new batch (but not if a batch is actively in progress)
    if (!anyWorking()) store.clear();
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

  i18n.onLocaleChange(() => { toolbar.refresh(); dropzone.refresh(); });

  listen('close-requested', async () => {
    if (anyWorking()) {
      const count = store.snapshot().filter((r) => r.status === 'working' || r.status === 'pending').length;
      if (!confirm(i18n.t('confirm.quitProcessing', { count }))) return;
    }
    await invoke('confirm_close');
  });
}
main();
