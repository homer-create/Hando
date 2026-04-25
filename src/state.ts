// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
export type FileStatus = 'pending' | 'working' | 'done' | 'error' | 'skipped-no-gain';

export interface FileRow {
  id: string;
  path: string;
  name: string;
  status: FileStatus;
  progress?: number;    // 0–100 while working, absent when done
  srcBytes?: number;
  outBytes?: number;
  errorMsg?: string;
}

type Listener = (rows: FileRow[]) => void;

class Store {
  private rows = new Map<string, FileRow>();
  private listeners = new Set<Listener>();

  snapshot(): FileRow[] { return Array.from(this.rows.values()); }
  subscribe(l: Listener): () => void { this.listeners.add(l); return () => this.listeners.delete(l); }
  upsert(row: FileRow) { this.rows.set(row.path, row); this.emit(); }
  update(path: string, patch: Partial<FileRow>) {
    const r = this.rows.get(path); if (!r) return;
    this.rows.set(path, { ...r, ...patch });
    this.emit();
  }
  snapshotById(id: string): FileRow | undefined {
    for (const r of this.rows.values()) if (r.id === id) return r;
    return undefined;
  }
  clear() { this.rows.clear(); this.emit(); }
  private emit() { const snap = this.snapshot(); for (const l of this.listeners) l(snap); }
}

export const store = new Store();

export function anyWorking(): boolean {
  for (const r of store.snapshot()) {
    if (r.status === 'working' || r.status === 'pending') return true;
  }
  return false;
}
