// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { PRESET_TARGETS, Settings } from './ui/settings';

export interface CompressFile { id: string; path: string; ext: string; }

export interface EncodeOpts {
  jpegQuality: number;
  pngQuality: number;
  webpQuality: number;
  avifQuality: number;
  emitWebp: boolean;
  emitAvif: boolean;
  avifSpeed: number;
  pngOxipngLevel: number;
  webpMethod: number;
  jpegProgressive: boolean;
  /** 'auto' = quality-targeted encoding against targetQuality; 'manual' = fixed sliders */
  mode: 'auto' | 'manual';
  /** ssimulacra2 target S for auto mode */
  targetQuality: number;
  /** keep EXIF metadata in outputs (default false = strip) */
  keepMetadata: boolean;
  /** keep ICC color profile in outputs (default true) */
  keepIcc: boolean;
}

export interface CompressArgs {
  batchId: string;
  files: CompressFile[];
  opts: EncodeOpts;
  moveOriginalsToTrash: boolean;
}

export async function compress(args: CompressArgs): Promise<void> {
  await invoke('compress', { args });
}

export interface FileDonePayload { id: string; srcBytes: number; outBytes: number; }
export interface FileErrorPayload { id: string; msg: string; }
export interface FileSkippedPayload { id: string; srcBytes: number; }
export interface BatchDonePayload { batchId: string; }

export interface FileProgressPayload { id: string; pct: number; }

export function onFileDone(cb: (p: FileDonePayload) => void) { return listen<FileDonePayload>('file-done', (e) => cb(e.payload)); }
export function onFileError(cb: (p: FileErrorPayload) => void) { return listen<FileErrorPayload>('file-error', (e) => cb(e.payload)); }
export function onFileSkipped(cb: (p: FileSkippedPayload) => void) { return listen<FileSkippedPayload>('file-skipped', (e) => cb(e.payload)); }
export function onBatchDone(cb: (p: BatchDonePayload) => void) { return listen<BatchDonePayload>('batch-done', (e) => cb(e.payload)); }
export function onFileProgress(cb: (p: FileProgressPayload) => void) { return listen<FileProgressPayload>('file-progress', (e) => cb(e.payload)); }

export function toOpts(s: Settings): EncodeOpts {
  return {
    jpegQuality: s.jpegQuality,
    pngQuality: s.pngQuality,
    webpQuality: s.webpQuality,
    avifQuality: s.avifQuality,
    emitWebp: s.emitWebp,
    emitAvif: s.emitAvif,
    avifSpeed: s.avifSpeed,
    pngOxipngLevel: s.pngOxipngLevel,
    webpMethod: s.webpMethod,
    jpegProgressive: s.jpegProgressive,
    mode: s.mode,
    targetQuality: PRESET_TARGETS[s.preset],
    keepMetadata: s.keepMetadata,
    keepIcc: s.keepIcc,
  };
}

export interface UndoReport { restored: number; attempted: number; }
export async function undoLastBatch(): Promise<UndoReport> {
  return invoke<UndoReport>('undo_last_batch');
}

export async function openTrash(): Promise<void> { await invoke('open_trash'); }
