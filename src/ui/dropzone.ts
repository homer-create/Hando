// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import { t } from '../i18n';

export interface DropzoneApi { refresh(): void; }

export async function mountDropzone(
  root: HTMLElement,
  onFiles: (paths: string[]) => void,
): Promise<DropzoneApi> {
  function render() {
    const link = `<span class="link" id="pick-files">${t('dropzone.clickToAdd')}</span>`;
    const prompt = t('dropzone.prompt', { link });
    root.innerHTML = `<div class="dropzone" id="dropzone">${prompt}</div>`;

    const dz = root.querySelector('#dropzone') as HTMLElement;
    dz.addEventListener('dragover', (e) => { e.preventDefault(); dz.classList.add('hover'); });
    dz.addEventListener('dragleave', () => dz.classList.remove('hover'));
    dz.addEventListener('drop', (e) => { e.preventDefault(); });

    (root.querySelector('#pick-files') as HTMLElement).addEventListener('click', async () => {
      const result = await open({
        multiple: true,
        filters: [{ name: t('dropzone.imagesFilter'), extensions: ['jpg', 'jpeg', 'png', 'webp'] }],
      });
      if (!result) return;
      const paths = Array.isArray(result) ? result : [result];
      if (paths.length) onFiles(paths);
    });
  }

  render();

  // Tauri 2.x native drop — registered exactly once, persists across re-renders
  await getCurrentWindow().onDragDropEvent((event) => {
    const dz = root.querySelector('#dropzone') as HTMLElement | null;
    if (event.payload.type === 'drop' && event.payload.paths.length > 0) {
      dz?.classList.remove('hover');
      onFiles(event.payload.paths);
    } else if (event.payload.type === 'leave') {
      dz?.classList.remove('hover');
    }
  });

  return { refresh: render };
}
