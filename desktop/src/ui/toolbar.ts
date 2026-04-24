// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
export interface ToolbarApi { setUndoEnabled(enabled: boolean): void; }
export function mountToolbar(
  root: HTMLElement,
  handlers: { onSettings: () => void; onUndo: () => void },
): ToolbarApi {
  root.innerHTML = `
    <div class="toolbar">
      <div class="title">ImageOpt</div>
      <div class="actions">
        <button id="btn-settings" class="btn">⚙ Settings</button>
        <button id="btn-undo" class="btn" disabled>↺ Undo</button>
      </div>
    </div>`;
  const undoBtn = root.querySelector('#btn-undo') as HTMLButtonElement;
  (root.querySelector('#btn-settings') as HTMLButtonElement).onclick = handlers.onSettings;
  undoBtn.onclick = handlers.onUndo;
  return { setUndoEnabled: (v: boolean) => { undoBtn.disabled = !v; } };
}
