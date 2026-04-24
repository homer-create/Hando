import { mountToolbar } from './ui/toolbar';
import { mountDropzone } from './ui/dropzone';
import { mountFileList } from './ui/file-list';
import { store } from './state';
import { basename } from './util/path';
import { openSettingsPanel, loadSettings, getSettings } from './ui/settings';
import { compress, toOpts, onFileDone, onFileError, onFileSkipped } from './ipc';

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

  mountToolbar(document.getElementById('toolbar')!, {
    onSettings: () => openSettingsPanel(),
    onUndo: () => console.log('undo clicked'),
  });

  mountDropzone(document.getElementById('dropzone')!, async (paths) => {
    const files = paths.map((p) => ({
      id: crypto.randomUUID(),
      path: p,
      ext: extOf(p),
      name: basename(p),
    }));
    for (const f of files) {
      store.upsert({ id: f.id, path: f.path, name: f.name, status: 'working' });
    }
    try {
      await compress({
        batchId: crypto.randomUUID(),
        files: files.map(({ id, path, ext }) => ({ id, path, ext })),
        opts: toOpts(getSettings()),
      });
    } catch (err) {
      console.error('compress failed:', err);
      for (const f of files) {
        store.update(f.path, { status: 'error', errorMsg: String(err) });
      }
    }
  });

  mountFileList(document.getElementById('list')!);
}
main();
