// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import en, { type Messages } from './locales/en';

export type LocaleCode = 'en' | 'zh-TW' | 'zh-CN' | 'ja' | 'ko' | 'es' | 'pt';
export type LanguageSetting = 'auto' | LocaleCode;

export const SUPPORTED_LOCALES: readonly LocaleCode[] = [
  'en', 'zh-TW', 'zh-CN', 'ja', 'ko', 'es', 'pt',
];

type Leaves<T, P extends string = ''> = {
  [K in keyof T]: T[K] extends string
    ? `${P}${K & string}`
    : Leaves<T[K], `${P}${K & string}.`>
}[keyof T];
export type MessageKey = Leaves<Messages>;

const LOCALES: Record<LocaleCode, Messages> = {
  'en': en,
  'zh-TW': en,   // placeholders — replaced in later tasks
  'zh-CN': en,
  'ja': en,
  'ko': en,
  'es': en,
  'pt': en,
};

let currentLocale: LocaleCode = 'en';
let currentMessages: Messages = en;
const listeners = new Set<(l: LocaleCode) => void>();

function resolveAuto(lang: string): LocaleCode {
  const l = lang.toLowerCase();
  if (l === 'zh-tw' || l.startsWith('zh-hant') || l === 'zh-hk' || l === 'zh-mo') return 'zh-TW';
  if (l === 'zh-cn' || l.startsWith('zh-hans') || l === 'zh-sg' || l === 'zh') return 'zh-CN';
  if (l.startsWith('ja')) return 'ja';
  if (l.startsWith('ko')) return 'ko';
  if (l.startsWith('es')) return 'es';
  if (l.startsWith('pt')) return 'pt';
  return 'en';
}

function resolve(setting: LanguageSetting): LocaleCode {
  if (setting !== 'auto') return setting;
  return resolveAuto(navigator.language || 'en');
}

/** Call once at application boot. Does not fire onLocaleChange listeners. */
export function init(setting: LanguageSetting): void {
  currentLocale = resolve(setting);
  currentMessages = LOCALES[currentLocale];
}

export function getLocale(): LocaleCode { return currentLocale; }

export function setLocale(setting: LanguageSetting): void {
  const next = resolve(setting);
  if (next === currentLocale) return;
  currentLocale = next;
  currentMessages = LOCALES[currentLocale];
  for (const cb of listeners) cb(currentLocale);
}

export function onLocaleChange(cb: (l: LocaleCode) => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

function getPath(obj: unknown, path: string): string | undefined {
  const parts = path.split('.');
  let cur: any = obj;
  for (const p of parts) {
    if (cur == null) return undefined;
    cur = cur[p];
  }
  return typeof cur === 'string' ? cur : undefined;
}

export function t(key: MessageKey, vars?: Record<string, string | number>): string {
  const raw = getPath(currentMessages, key) ?? getPath(en, key) ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (_, k) => {
    const v = vars[k];
    return v === undefined ? `{${k}}` : String(v);
  });
}

let cachedFmtLocale: LocaleCode | undefined;
let cachedFmt: Intl.NumberFormat;
function getNumFmt(): Intl.NumberFormat {
  if (cachedFmtLocale !== currentLocale) {
    cachedFmtLocale = currentLocale;
    cachedFmt = new Intl.NumberFormat(currentLocale, { maximumFractionDigits: 1 });
  }
  return cachedFmt;
}

export function fmtBytes(n: number): string {
  const fmt = getNumFmt();
  if (n < 1024) return `${fmt.format(n)} B`;
  if (n < 1024 * 1024) return `${fmt.format(n / 1024)} KB`;
  return `${fmt.format(n / 1024 / 1024)} MB`;
}
