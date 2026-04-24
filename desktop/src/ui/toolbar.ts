export function mountToolbar(root: HTMLElement, handlers: { onSettings: () => void; onUndo: () => void }) {
  root.innerHTML = `
    <div class="toolbar">
      <div class="title">ImageOpt</div>
      <div class="actions">
        <button id="btn-settings" class="btn">⚙ Settings</button>
        <button id="btn-undo" class="btn" disabled>↺ Undo</button>
      </div>
    </div>`;
  (root.querySelector('#btn-settings') as HTMLButtonElement).onclick = handlers.onSettings;
  (root.querySelector('#btn-undo') as HTMLButtonElement).onclick = handlers.onUndo;
}
