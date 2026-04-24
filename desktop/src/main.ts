import { mountToolbar } from './ui/toolbar';
import { mountDropzone } from './ui/dropzone';
import { mountFileList } from './ui/file-list';
import { store } from './state';
import { basename } from './util/path';
import { openSettingsPanel, loadSettings } from './ui/settings';

async function main() {
  await loadSettings();

  mountToolbar(document.getElementById('toolbar')!, {
    onSettings: () => openSettingsPanel(),
    onUndo: () => console.log('undo clicked'),
  });

  mountDropzone(document.getElementById('dropzone')!, (paths) => {
    for (const p of paths) {
      store.upsert({
        id: crypto.randomUUID(),
        path: p,
        name: basename(p),
        status: 'pending',
      });
    }
  });

  mountFileList(document.getElementById('list')!);
}
main();
