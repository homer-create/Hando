// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { store, FileRow } from '../state';
import { openTrash } from '../ipc';
import { t, fmtBytes, onLocaleChange } from '../i18n';

export function mountStatusBar(root: HTMLElement) {
  root.innerHTML = `<span id="sb-left"></span><span id="sb-right"></span>`;
  const left = root.querySelector('#sb-left') as HTMLElement;
  const right = root.querySelector('#sb-right') as HTMLElement;

  function renderRight() {
    right.innerHTML = `${t('statusbar.trashHint')} · <span class="link" id="show-trash">${t('statusbar.trashShow')}</span>`;
    (right.querySelector('#show-trash') as HTMLElement).onclick = () => { openTrash(); };
  }

  function render(rows: FileRow[]) {
    const total = rows.length;
    const working = rows.filter((r) => r.status === 'working' || r.status === 'pending').length;
    const completed = rows.filter((r) => r.status === 'done' || r.status === 'error' || r.status === 'skipped-no-gain').length;

    if (total > 0 && working > 0) {
      const pct = Math.round((completed / total) * 100);
      left.innerHTML = `
        <span class="progress-wrap">
          <span class="progress-bar ${working > 0 ? 'progress-bar--active' : ''}" style="width:${pct}%"></span>
        </span>
        <span class="progress-label">${t('statusbar.progress', { completed, total, pct })}</span>`;
    } else {
      let saved = 0;
      let done = 0;
      for (const r of rows) {
        if (r.status === 'done' && r.srcBytes && r.outBytes) {
          saved += r.srcBytes - r.outBytes;
          done++;
        }
      }
      left.innerHTML = done > 0
        ? t('statusbar.saved', { amount: fmtBytes(saved), count: done })
        : '';
    }
  }

  renderRight();
  store.subscribe(render);
  onLocaleChange(() => { renderRight(); render(store.snapshot()); });
  render(store.snapshot());
}
