// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { t } from '../i18n';

export interface ToolbarApi {
  setUndoEnabled(enabled: boolean): void;
  refresh(): void;
}

export function mountToolbar(
  root: HTMLElement,
  handlers: { onSettings: () => void; onUndo: () => void },
): ToolbarApi {
  let undoEnabled = false;

  function render() {
    root.innerHTML = `
      <div class="toolbar">
        <div class="title">Hando</div>
        <div class="actions">
          <button id="btn-settings" class="btn">${t('toolbar.settings')}</button>
          <button id="btn-undo" class="btn"${undoEnabled ? '' : ' disabled'}>${t('toolbar.undo')}</button>
        </div>
      </div>`;
    (root.querySelector('#btn-settings') as HTMLButtonElement).onclick = handlers.onSettings;
    (root.querySelector('#btn-undo') as HTMLButtonElement).onclick = handlers.onUndo;
  }

  render();

  return {
    setUndoEnabled(v: boolean) {
      undoEnabled = v;
      const btn = root.querySelector('#btn-undo') as HTMLButtonElement | null;
      if (btn) btn.disabled = !v;
    },
    refresh: render,
  };
}
