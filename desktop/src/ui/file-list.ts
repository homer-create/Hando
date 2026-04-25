// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { store, FileRow } from '../state';
import { t, fmtBytes, onLocaleChange } from '../i18n';

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
      list.innerHTML = `<div class="empty">${t('fileList.empty')}</div>`;
      return;
    }
    list.innerHTML = rows.map((r) => `
      <div class="row status-${r.status}" title="${r.path}">
        <span class="icon">${statusIcon(r)}</span>
        <span class="name">${r.name}</span>
        <span class="size old">${r.srcBytes !== undefined ? fmtBytes(r.srcBytes) : ''}</span>
        <span class="size new">${r.outBytes !== undefined ? fmtBytes(r.outBytes) : ''}</span>
        <span class="savings">${savings(r)}</span>
      </div>
    `).join('');
  }

  store.subscribe(render);
  onLocaleChange(() => render(store.snapshot()));
  render(store.snapshot());
}
