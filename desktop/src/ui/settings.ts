export interface Settings {
  jpegQuality: number;
  pngQuality: number;
  webpQuality: number;
  emitWebp: boolean;
  emitAvif: boolean;            // disabled in UI for v1
  moveOriginalsToTrash: boolean;
  concurrency: number;
}

export const DEFAULT_SETTINGS: Settings = {
  jpegQuality: 75,
  pngQuality: 75,
  webpQuality: 75,
  emitWebp: false,
  emitAvif: false,
  moveOriginalsToTrash: true,
  concurrency: 4,
};

let current: Settings = { ...DEFAULT_SETTINGS };

export function getSettings(): Settings { return { ...current }; }
export function setSettings(next: Settings) { current = { ...next }; }

export function openSettingsPanel(onChange: (s: Settings) => void) {
  const existing = document.getElementById('settings-overlay');
  if (existing) existing.remove();
  const overlay = document.createElement('div');
  overlay.id = 'settings-overlay';
  overlay.innerHTML = `
    <div class="settings-panel">
      <h2>Settings</h2>
      <label>JPEG quality <span id="v-jpeg">${current.jpegQuality}</span></label>
      <input type="range" min="1" max="100" id="jpeg" value="${current.jpegQuality}" />
      <label>PNG quality <span id="v-png">${current.pngQuality}</span></label>
      <input type="range" min="1" max="100" id="png" value="${current.pngQuality}" />
      <label>WebP quality <span id="v-webp">${current.webpQuality}</span></label>
      <input type="range" min="1" max="100" id="webp" value="${current.webpQuality}" />
      <label><input type="checkbox" id="emitWebp" ${current.emitWebp ? 'checked' : ''} /> Also emit WebP alongside</label>
      <label><input type="checkbox" id="emitAvif" disabled /> Also emit AVIF (coming soon)</label>
      <label><input type="checkbox" id="trash" ${current.moveOriginalsToTrash ? 'checked' : ''} /> Move originals to Trash</label>
      <div class="settings-actions"><button id="done" class="btn">Done</button></div>
    </div>`;
  document.body.appendChild(overlay);

  const bindRange = (id: string, key: keyof Settings) => {
    const el = overlay.querySelector(`#${id}`) as HTMLInputElement;
    const label = overlay.querySelector(`#v-${id}`) as HTMLElement;
    el.oninput = () => { label.textContent = el.value; (current as any)[key] = parseInt(el.value, 10); onChange({ ...current }); };
  };
  bindRange('jpeg', 'jpegQuality');
  bindRange('png', 'pngQuality');
  bindRange('webp', 'webpQuality');
  (overlay.querySelector('#emitWebp') as HTMLInputElement).onchange = (e) => { current.emitWebp = (e.target as HTMLInputElement).checked; onChange({ ...current }); };
  (overlay.querySelector('#trash') as HTMLInputElement).onchange = (e) => { current.moveOriginalsToTrash = (e.target as HTMLInputElement).checked; onChange({ ...current }); };
  (overlay.querySelector('#done') as HTMLButtonElement).onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
}
