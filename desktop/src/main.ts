import { mountToolbar } from './ui/toolbar';
import { mountDropzone } from './ui/dropzone';
import { mountFileList } from './ui/file-list';
import { store } from './state';
import { basename } from './util/path';

mountToolbar(document.getElementById('toolbar')!, {
  onSettings: () => console.log('settings clicked'),
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
