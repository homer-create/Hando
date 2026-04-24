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
    const total = rows.length;
    const working = rows.filter((r) => r.status === 'working' || r.status === 'pending').length;
    const completed = rows.filter((r) => r.status === 'done' || r.status === 'error' || r.status === 'skipped-no-gain').length;

    if (total > 0 && working > 0) {
      const pct = Math.round((completed / total) * 100);
      left.innerHTML = `
        <span class="progress-wrap">
          <span class="progress-bar" style="width:${pct}%"></span>
        </span>
        <span class="progress-label">${completed} / ${total} 張 (${pct}%)</span>`;
    } else {
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
  }
  store.subscribe(render);
  render(store.snapshot());
}
