import { store, FileRow } from '../state';

function fmtBytes(n?: number): string {
  if (n === undefined) return '';
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function statusIcon(row: FileRow): string {
  switch (row.status) {
    case 'done': return '✓';
    case 'working': return '⏵';
    case 'error': return '⚠';
    case 'skipped-no-gain': return '=';
    default: return '○';
  }
}

function savings(row: FileRow): string {
  if (row.status !== 'done' || !row.srcBytes || !row.outBytes) return '';
  const pct = Math.round((1 - row.outBytes / row.srcBytes) * 100);
  return `−${pct}%`;
}

export function mountFileList(root: HTMLElement) {
  root.innerHTML = `<div class="file-list" id="file-list"></div>`;
  const list = root.querySelector('#file-list') as HTMLElement;

  function render(rows: FileRow[]) {
    if (rows.length === 0) {
      list.innerHTML = `<div class="empty">No files yet. Drag images onto the window.</div>`;
      return;
    }
    list.innerHTML = rows.map((r) => `
      <div class="row status-${r.status}" title="${r.path}">
        <span class="icon">${statusIcon(r)}</span>
        <span class="name">${r.name}</span>
        <span class="size old">${fmtBytes(r.srcBytes)}</span>
        <span class="size new">${fmtBytes(r.outBytes)}</span>
        <span class="savings">${savings(r)}</span>
      </div>
    `).join('');
  }

  store.subscribe(render);
  render(store.snapshot());
}
