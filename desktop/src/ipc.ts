import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Settings } from './ui/settings';

export interface CompressFile { id: string; path: string; ext: string; }

export interface CompressArgs {
  batchId: string;
  files: CompressFile[];
  opts: {
    jpegQuality: number;
    pngQuality: number;
    webpQuality: number;
    emitWebp: boolean;
    emitAvif: boolean;
  };
}

export async function compress(args: CompressArgs): Promise<void> {
  await invoke('compress', { args });
}

export interface FileDonePayload { id: string; tmp: string; srcBytes: number; outBytes: number; }
export interface FileErrorPayload { id: string; msg: string; }
export interface FileSkippedPayload { id: string; srcBytes: number; }

export function onFileDone(cb: (p: FileDonePayload) => void) { return listen<FileDonePayload>('file-done', (e) => cb(e.payload)); }
export function onFileError(cb: (p: FileErrorPayload) => void) { return listen<FileErrorPayload>('file-error', (e) => cb(e.payload)); }
export function onFileSkipped(cb: (p: FileSkippedPayload) => void) { return listen<FileSkippedPayload>('file-skipped', (e) => cb(e.payload)); }

export function toOpts(s: Settings) {
  return {
    jpegQuality: s.jpegQuality,
    pngQuality: s.pngQuality,
    webpQuality: s.webpQuality,
    emitWebp: s.emitWebp,
    emitAvif: s.emitAvif,
  };
}
