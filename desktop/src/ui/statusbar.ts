import { store, FileRow } from '../state';
import { openTrash } from '../ipc';

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export function mountStatusBar(root: HTMLElement) {
  root.innerHTML = `<span id="sb-left"></span><span id="sb-right"></span>`;
  const left = root.querySelector('#sb-left') as HTMLElement;
  const right = root.querySelector('#sb-right') as HTMLElement;
  right.innerHTML = `Originals moved to Trash · <span class="link" id="show-trash">Show</span>`;
  (right.querySelector('#show-trash') as HTMLElement).onclick = () => { openTrash(); };

  function render(rows: FileRow[]) {
    let saved = 0;
    let done = 0;
    for (const r of rows) {
      if (r.status === 'done' && r.srcBytes && r.outBytes) {
        saved += r.srcBytes - r.outBytes;
        done++;
      }
    }
    left.innerHTML = done > 0 ? `Saved <b class="good">${fmtBytes(saved)}</b> across ${done} files` : '';
  }
  store.subscribe(render);
  render(store.snapshot());
}
