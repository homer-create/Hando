// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { getSettings, saveSettings } from './settings';

export type ThemePref = 'auto' | 'light' | 'dark';

const mq = window.matchMedia('(prefers-color-scheme: dark)');
let mediaListener: ((e: MediaQueryListEvent) => void) | null = null;

function applyResolved(pref: ThemePref): void {
  if (mediaListener) {
    mq.removeEventListener('change', mediaListener);
    mediaListener = null;
  }
  const dark = pref === 'dark' || (pref === 'auto' && mq.matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  if (pref === 'auto') {
    mediaListener = (e) => {
      document.documentElement.dataset.theme = e.matches ? 'dark' : 'light';
    };
    mq.addEventListener('change', mediaListener);
  }
}

export function initTheme(): void {
  applyResolved(getSettings().theme ?? 'auto');
}

export function setThemePref(pref: ThemePref): void {
  applyResolved(pref);
  const s = getSettings();
  s.theme = pref;
  saveSettings(s);
}
