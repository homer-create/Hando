import { Store } from '@tauri-apps/plugin-store';

export interface Settings {
  jpegQuality: number;
  pngQuality: number;
  webpQuality: number;
  avifQuality: number;
  emitWebp: boolean;
  emitAvif: boolean;
  moveOriginalsToTrash: boolean;
  concurrency: number;
}

export const DEFAULT_SETTINGS: Settings = {
  jpegQuality: 75,
  pngQuality: 75,
  webpQuality: 75,
  avifQuality: 50,
  emitWebp: false,
  emitAvif: false,
  moveOriginalsToTrash: true,
  concurrency: 4,
};

const SETTINGS_FILE = 'settings.json';
let storePromise: Promise<Store> | null = null;

function getStore(): Promise<Store> {
  if (!storePromise) storePromise = Store.load(SETTINGS_FILE);
  return storePromise;
}

let current: Settings = { ...DEFAULT_SETTINGS };

export async function loadSettings(): Promise<Settings> {
  try {
    const s = await getStore();
    const persisted = await s.get<Settings>('settings');
    if (persisted) current = { ...DEFAULT_SETTINGS, ...persisted };
  } catch (err) {
    console.warn('settings load failed, using defaults:', err);
  }
  return { ...current };
}

export async function saveSettings(next: Settings): Promise<void> {
  current = { ...next };
  try {
    const s = await getStore();
    await s.set('settings', current);
    await s.save();
  } catch (err) {
    console.error('settings save failed:', err);
  }
}

export function getSettings(): Settings { return { ...current }; }

export function openSettingsPanel() {
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
      <label>AVIF quality <span id="v-avif">${current.avifQuality}</span></label>
      <input type="range" min="1" max="100" id="avif" value="${current.avifQuality}" />
      <label><input type="checkbox" id="emitWebp" ${current.emitWebp ? 'checked' : ''} /> Also emit WebP alongside</label>
      <label><input type="checkbox" id="emitAvif" ${current.emitAvif ? 'checked' : ''} /> Also emit AVIF alongside</label>
      <label><input type="checkbox" id="trash" ${current.moveOriginalsToTrash ? 'checked' : ''} /> Move originals to Trash</label>
      <div class="settings-actions"><button id="done" class="btn">Done</button></div>
    </div>`;
  document.body.appendChild(overlay);

  const bindRange = (id: string, key: keyof Settings) => {
    const el = overlay.querySelector(`#${id}`) as HTMLInputElement;
    const label = overlay.querySelector(`#v-${id}`) as HTMLElement;
    el.oninput = () => {
      label.textContent = el.value;
      (current as any)[key] = parseInt(el.value, 10);
      saveSettings(current);
    };
  };
  bindRange('jpeg', 'jpegQuality');
  bindRange('png', 'pngQuality');
  bindRange('webp', 'webpQuality');
  bindRange('avif', 'avifQuality');
  (overlay.querySelector('#emitWebp') as HTMLInputElement).onchange = (e) => { current.emitWebp = (e.target as HTMLInputElement).checked; saveSettings(current); };
  (overlay.querySelector('#emitAvif') as HTMLInputElement).onchange = (e) => { current.emitAvif = (e.target as HTMLInputElement).checked; saveSettings(current); };
  (overlay.querySelector('#trash') as HTMLInputElement).onchange = (e) => { current.moveOriginalsToTrash = (e.target as HTMLInputElement).checked; saveSettings(current); };
  (overlay.querySelector('#done') as HTMLButtonElement).onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
}
