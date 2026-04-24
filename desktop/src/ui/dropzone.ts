export function mountDropzone(root: HTMLElement, onFiles: (paths: string[]) => void) {
  root.innerHTML = `<div class="dropzone" id="dropzone">Drag images here, or <span class="link">click to add</span></div>`;
  const dz = root.querySelector('#dropzone') as HTMLElement;

  dz.addEventListener('dragover', (e) => { e.preventDefault(); dz.classList.add('hover'); });
  dz.addEventListener('dragleave', () => dz.classList.remove('hover'));
  dz.addEventListener('drop', (e) => {
    e.preventDefault();
    dz.classList.remove('hover');
    const paths: string[] = [];
    for (const f of Array.from(e.dataTransfer?.files ?? [])) {
      // @ts-expect-error Tauri webview exposes .path on dropped files
      if (f.path) paths.push(f.path);
    }
    if (paths.length) onFiles(paths);
  });
}
