import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';

export async function mountDropzone(root: HTMLElement, onFiles: (paths: string[]) => void): Promise<void> {
  root.innerHTML = `<div class="dropzone" id="dropzone">Drag images here, or <span class="link" id="pick-files">click to add</span></div>`;
  const dz = root.querySelector('#dropzone') as HTMLElement;

  // HTML5 dragover/dragleave for visual hover feedback only
  dz.addEventListener('dragover', (e) => { e.preventDefault(); dz.classList.add('hover'); });
  dz.addEventListener('dragleave', () => dz.classList.remove('hover'));
  dz.addEventListener('drop', (e) => { e.preventDefault(); }); // prevent browser default

  // Tauri 2.x native drop — this is where we get actual file paths
  await getCurrentWindow().onDragDropEvent((event) => {
    if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
      dz.classList.remove('hover');
      onFiles(event.payload.paths);
    } else if (event.payload.type === 'leave') {
      dz.classList.remove('hover');
    }
  });

  // Click-to-add via system file picker
  (root.querySelector('#pick-files') as HTMLElement).addEventListener('click', async () => {
    const result = await open({
      multiple: true,
      filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp'] }],
    });
    if (!result) return;
    const paths = Array.isArray(result) ? result : [result];
    if (paths.length) onFiles(paths);
  });
}
