// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

export interface Messages {
  toolbar: { settings: string; undo: string };
  dropzone: { prompt: string; clickToAdd: string; imagesFilter: string };
  fileList: { empty: string; skipped: string };
  statusbar: { progress: string; saved: string; trashHint: string; trashShow: string };
  settings: {
    title: string;
    language: string;
    languageAuto: string;
    jpegQuality: string;
    pngQuality: string;
    webpQuality: string;
    avifQuality: string;
    emitWebp: string;
    emitAvif: string;
    moveToTrash: string;
    done: string;
    theme: string;
    themeAuto: string;
    themeLight: string;
    mode: string;
    modeAuto: string;
    modeManual: string;
    preset: string;
    presetLossless: string;
    presetBalanced: string;
    presetAggressive: string;
    advanced: string;
    avifSpeed: string;
    oxipngLevel: string;
    webpMethod: string;
    jpegProgressive: string;
    themeDark: string;
  };
  confirm: { quitProcessing: string };
  alert: { engineCrashed: string };
}

const messages: Messages = {
  toolbar: { settings: '⚙ Settings', undo: '↺ Undo' },
  dropzone: {
    prompt: 'Drag images here, or {link}',
    clickToAdd: 'click to add',
    imagesFilter: 'Images',
  },
  fileList: { empty: 'No files yet. Drag images onto the window.', skipped: 'Already optimized' },
  statusbar: {
    progress: '{completed} / {total} files ({pct}%)',
    saved: 'Saved {amount} across {count} files',
    trashHint: 'Originals moved to Trash',
    trashShow: 'Show',
  },
  settings: {
    title: 'Settings',
    language: 'Language',
    languageAuto: 'Auto detect (system)',
    jpegQuality: 'JPEG quality',
    pngQuality: 'PNG quality',
    webpQuality: 'WebP quality',
    avifQuality: 'AVIF quality',
    emitWebp: 'Also emit WebP alongside',
    emitAvif: 'Also emit AVIF alongside',
    moveToTrash: 'Move originals to Trash',
    done: 'Done',
    theme: 'Theme',
    themeAuto: 'Auto',
    themeLight: 'Light',
    mode: 'Mode',
    modeAuto: 'Auto (quality target)',
    modeManual: 'Manual',
    preset: 'Quality target',
    presetLossless: 'Visually lossless',
    presetBalanced: 'Balanced',
    presetAggressive: 'Aggressive',
    advanced: 'Advanced',
    avifSpeed: 'AVIF speed',
    oxipngLevel: 'PNG effort (oxipng)',
    webpMethod: 'WebP effort (method)',
    jpegProgressive: 'Progressive JPEG',
    themeDark: 'Dark',
  },
  confirm: { quitProcessing: '{count} files still processing. Quit anyway?' },
  alert: { engineCrashed: 'Image engine crashed. It will restart on the next drop.' },
};

export default messages;
