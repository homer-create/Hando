// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { Store } from '@tauri-apps/plugin-store';
import { setLocale, t, type LanguageSetting } from '../i18n';
import { setThemePref, type ThemePref } from './theme';

export type EncodeMode = 'auto' | 'manual';
export type QualityPreset = 'lossless' | 'balanced' | 'aggressive';

/** ssimulacra2 target S per preset — provisional values pending human
 *  calibration, see docs/calibration.md */
export const PRESET_TARGETS: Record<QualityPreset, number> = {
  lossless: 90,
  balanced: 80,
  aggressive: 70,
};

export interface Settings {
  mode: EncodeMode;
  preset: QualityPreset;
  jpegQuality: number;
  pngQuality: number;
  webpQuality: number;
  avifQuality: number;
  emitWebp: boolean;
  emitAvif: boolean;
  avifSpeed: number;       // ravif 1 (smallest) – 10 (fastest)
  pngOxipngLevel: number;  // oxipng preset 0–6
  webpMethod: number;      // libwebp method 0 (fastest) – 6 (smallest)
  jpegProgressive: boolean;
  keepMetadata: boolean;   // EXIF passthrough (off = strip, the privacy-safe default)
  keepIcc: boolean;        // ICC color profile passthrough
  moveOriginalsToTrash: boolean;
  concurrency: number;
  language: LanguageSetting;
  theme: ThemePref;
}

export const DEFAULT_SETTINGS: Settings = {
  mode: 'auto',
  preset: 'lossless',
  jpegQuality: 75,
  pngQuality: 75,
  webpQuality: 75,
  avifQuality: 50,
  emitWebp: false,
  emitAvif: false,
  avifSpeed: 8,
  pngOxipngLevel: 4,
  webpMethod: 4,
  jpegProgressive: true,
  keepMetadata: false,
  keepIcc: true,
  moveOriginalsToTrash: true,
  concurrency: 4,
  language: 'auto',
  theme: 'auto',
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

type SettingsTab = 'general' | 'compression' | 'output';
/** Survives panel rebuilds (e.g. the locale-change re-render). */
let activeTab: SettingsTab = 'general';

export function openSettingsPanel() {
  const existing = document.getElementById('settings-overlay');
  if (existing) existing.remove();
  const overlay = document.createElement('div');
  overlay.id = 'settings-overlay';
  const tabBtn = (id: SettingsTab, label: string) =>
    `<button class="tab-btn${activeTab === id ? ' active' : ''}" data-tab="${id}">${label}</button>`;
  const pane = (id: SettingsTab) =>
    `class="tab-pane${activeTab === id ? ' active' : ''}" data-pane="${id}"`;
  overlay.innerHTML = `
    <div class="settings-panel">
      <h2>${t('settings.title')}</h2>
      <div class="settings-tabs" id="settings-tabs">
        ${tabBtn('general', t('settings.tabGeneral'))}
        ${tabBtn('compression', t('settings.tabCompression'))}
        ${tabBtn('output', t('settings.tabOutput'))}
      </div>
      <div ${pane('general')}>
        <label>${t('settings.theme')}</label>
        <div class="theme-seg" id="theme-seg">
          <button class="seg-btn${current.theme === 'auto' ? ' active' : ''}" data-value="auto">${t('settings.themeAuto')}</button>
          <button class="seg-btn${current.theme === 'light' ? ' active' : ''}" data-value="light">${t('settings.themeLight')}</button>
          <button class="seg-btn${current.theme === 'dark' ? ' active' : ''}" data-value="dark">${t('settings.themeDark')}</button>
        </div>
        <label>${t('settings.language')}</label>
        <select id="lang">
          <option value="auto"${current.language === 'auto' ? ' selected' : ''}>${t('settings.languageAuto')}</option>
          <option value="zh-TW"${current.language === 'zh-TW' ? ' selected' : ''}>繁體中文</option>
          <option value="zh-CN"${current.language === 'zh-CN' ? ' selected' : ''}>简体中文</option>
          <option value="en"${current.language === 'en' ? ' selected' : ''}>English</option>
          <option value="ja"${current.language === 'ja' ? ' selected' : ''}>日本語</option>
          <option value="ko"${current.language === 'ko' ? ' selected' : ''}>한국어</option>
          <option value="es"${current.language === 'es' ? ' selected' : ''}>Español</option>
          <option value="pt"${current.language === 'pt' ? ' selected' : ''}>Português</option>
        </select>
      </div>
      <div ${pane('compression')}>
        <label>${t('settings.mode')}</label>
        <div class="theme-seg" id="mode-seg">
          <button class="seg-btn${current.mode === 'auto' ? ' active' : ''}" data-value="auto">${t('settings.modeAuto')}</button>
          <button class="seg-btn${current.mode === 'manual' ? ' active' : ''}" data-value="manual">${t('settings.modeManual')}</button>
        </div>
        <div id="auto-section" style="display:${current.mode === 'auto' ? '' : 'none'}">
          <label>${t('settings.preset')}</label>
          <div class="theme-seg" id="preset-seg">
            <button class="seg-btn${current.preset === 'lossless' ? ' active' : ''}" data-value="lossless">${t('settings.presetLossless')}</button>
            <button class="seg-btn${current.preset === 'balanced' ? ' active' : ''}" data-value="balanced">${t('settings.presetBalanced')}</button>
            <button class="seg-btn${current.preset === 'aggressive' ? ' active' : ''}" data-value="aggressive">${t('settings.presetAggressive')}</button>
          </div>
        </div>
        <div id="manual-section" style="display:${current.mode === 'manual' ? '' : 'none'}">
          <p class="settings-hint">${t('settings.manualHint')}</p>
          <label>${t('settings.jpegQuality')} <span id="v-jpeg">${current.jpegQuality}</span></label>
          <input type="range" min="1" max="100" id="jpeg" value="${current.jpegQuality}" />
          <label>${t('settings.pngQuality')} <span id="v-png">${current.pngQuality}</span></label>
          <input type="range" min="1" max="100" id="png" value="${current.pngQuality}" />
          <label>${t('settings.webpQuality')} <span id="v-webp">${current.webpQuality}</span></label>
          <input type="range" min="1" max="100" id="webp" value="${current.webpQuality}" />
          <label>${t('settings.avifQuality')} <span id="v-avif">${current.avifQuality}</span></label>
          <input type="range" min="1" max="100" id="avif" value="${current.avifQuality}" />
          <details class="settings-advanced">
            <summary>${t('settings.advanced')}</summary>
            <p class="settings-hint">${t('settings.advancedHint')}</p>
            <label>${t('settings.avifSpeed')} <span id="v-avifSpeed">${current.avifSpeed}</span></label>
            <input type="range" min="1" max="10" id="avifSpeed" value="${current.avifSpeed}" />
            <p class="settings-hint">${t('settings.avifSpeedHint')}</p>
            <label>${t('settings.oxipngLevel')} <span id="v-oxipng">${current.pngOxipngLevel}</span></label>
            <input type="range" min="0" max="6" id="oxipng" value="${current.pngOxipngLevel}" />
            <p class="settings-hint">${t('settings.oxipngLevelHint')}</p>
            <label>${t('settings.webpMethod')} <span id="v-webpMethod">${current.webpMethod}</span></label>
            <input type="range" min="0" max="6" id="webpMethod" value="${current.webpMethod}" />
            <p class="settings-hint">${t('settings.webpMethodHint')}</p>
            <label><input type="checkbox" id="jpegProgressive" ${current.jpegProgressive ? 'checked' : ''} /> ${t('settings.jpegProgressive')}</label>
            <p class="settings-hint">${t('settings.jpegProgressiveHint')}</p>
          </details>
        </div>
      </div>
      <div ${pane('output')}>
        <label><input type="checkbox" id="emitWebp" ${current.emitWebp ? 'checked' : ''} /> ${t('settings.emitWebp')}</label>
        <label><input type="checkbox" id="emitAvif" ${current.emitAvif ? 'checked' : ''} /> ${t('settings.emitAvif')}</label>
        <label><input type="checkbox" id="keepMetadata" ${current.keepMetadata ? 'checked' : ''} /> ${t('settings.keepMetadata')}</label>
        <p class="settings-hint">${t('settings.keepMetadataHint')}</p>
        <label><input type="checkbox" id="keepIcc" ${current.keepIcc ? 'checked' : ''} /> ${t('settings.keepIcc')}</label>
        <p class="settings-hint">${t('settings.keepIccHint')}</p>
        <label><input type="checkbox" id="trash" ${current.moveOriginalsToTrash ? 'checked' : ''} /> ${t('settings.moveToTrash')}</label>
      </div>
      <div class="settings-actions"><button id="done" class="btn">${t('settings.done')}</button></div>
    </div>`;
  document.body.appendChild(overlay);

  (overlay.querySelector('#settings-tabs') as HTMLElement).addEventListener('click', (e) => {
    const btn = (e.target as HTMLElement).closest('[data-tab]') as HTMLElement | null;
    if (!btn) return;
    activeTab = btn.dataset.tab as SettingsTab;
    overlay.querySelectorAll('.tab-btn').forEach((b) => b.classList.toggle('active', b === btn));
    overlay.querySelectorAll('.tab-pane').forEach((p) =>
      p.classList.toggle('active', (p as HTMLElement).dataset.pane === activeTab));
  });

  (overlay.querySelector('#lang') as HTMLSelectElement).onchange = (e) => {
    const value = (e.target as HTMLSelectElement).value as LanguageSetting;
    current.language = value;
    saveSettings(current);
    setLocale(value);
    // Rebuild the open panel so it reflects the new locale immediately
    openSettingsPanel();
  };

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
  bindRange('avifSpeed', 'avifSpeed');
  bindRange('oxipng', 'pngOxipngLevel');
  bindRange('webpMethod', 'webpMethod');

  const bindSeg = (segId: string, apply: (value: string, btn: HTMLElement) => void) => {
    (overlay.querySelector(`#${segId}`) as HTMLElement).addEventListener('click', (e) => {
      const btn = (e.target as HTMLElement).closest('[data-value]') as HTMLElement | null;
      if (!btn) return;
      apply(btn.dataset.value!, btn);
      btn.parentElement!.querySelectorAll('.seg-btn').forEach((b) => b.classList.toggle('active', b === btn));
      saveSettings(current);
    });
  };
  bindSeg('mode-seg', (value) => {
    current.mode = value as EncodeMode;
    (overlay.querySelector('#auto-section') as HTMLElement).style.display = value === 'auto' ? '' : 'none';
    (overlay.querySelector('#manual-section') as HTMLElement).style.display = value === 'manual' ? '' : 'none';
  });
  bindSeg('preset-seg', (value) => { current.preset = value as QualityPreset; });

  const bindCheckbox = (id: string, key: keyof Settings) => {
    (overlay.querySelector(`#${id}`) as HTMLInputElement).onchange = (e) => {
      (current as any)[key] = (e.target as HTMLInputElement).checked;
      saveSettings(current);
    };
  };
  bindCheckbox('jpegProgressive', 'jpegProgressive');
  bindCheckbox('emitWebp', 'emitWebp');
  bindCheckbox('emitAvif', 'emitAvif');
  bindCheckbox('keepMetadata', 'keepMetadata');
  bindCheckbox('keepIcc', 'keepIcc');
  bindCheckbox('trash', 'moveOriginalsToTrash');
  (overlay.querySelector('#done') as HTMLButtonElement).onclick = () => overlay.remove();
  overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
  (overlay.querySelector('#theme-seg') as HTMLElement).addEventListener('click', (e) => {
    const btn = (e.target as HTMLElement).closest('[data-value]') as HTMLElement | null;
    if (!btn) return;
    const pref = btn.dataset.value as ThemePref;
    current.theme = pref;
    setThemePref(pref);
    // scope to this segment — an overlay-wide toggle would clear the
    // mode/preset segments' active state
    btn.parentElement!.querySelectorAll('.seg-btn').forEach((b) => b.classList.toggle('active', b === btn));
  });
}
