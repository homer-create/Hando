// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

export interface Messages {
  toolbar: { settings: string; undo: string };
  dropzone: { prompt: string; clickToAdd: string; imagesFilter: string };
  fileList: { empty: string; skipped: string };
  statusbar: { progress: string; saved: string; trashHint: string; trashShow: string };
  settings: {
    title: string;
    tabGeneral: string;
    tabCompression: string;
    tabOutput: string;
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
    advancedHint: string;
    manualHint: string;
    avifSpeed: string;
    avifSpeedHint: string;
    oxipngLevel: string;
    oxipngLevelHint: string;
    webpMethod: string;
    webpMethodHint: string;
    jpegProgressive: string;
    jpegProgressiveHint: string;
    keepMetadata: string;
    keepMetadataHint: string;
    keepIcc: string;
    keepIccHint: string;
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
    tabGeneral: 'General',
    tabCompression: 'Compression',
    tabOutput: 'Output',
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
    advancedHint: 'These only affect file size and encoding speed — they never change image quality.',
    manualHint: 'Manual mode applies your quality settings directly, with no automatic quality check; re-compressing already-compressed images accumulates quality loss.',
    avifSpeed: 'AVIF encoding speed',
    avifSpeedHint: 'Slower = smaller files but a longer wait; faster = saves time, slightly larger files. No effect on quality.',
    oxipngLevel: 'PNG compression effort',
    oxipngLevelHint: 'How hard PNG lossless compression tries. Higher = a bit smaller but slower; quality is completely unchanged.',
    webpMethod: 'WebP compression effort',
    webpMethodHint: 'How hard the WebP encoder tries. Higher = smaller but slower. No effect on quality.',
    jpegProgressive: 'JPEG progressive loading',
    jpegProgressiveHint: 'On: the image sharpens gradually as it loads on the web, and is usually a bit smaller (recommended). Off: faster encoding but noticeably larger files.',
    keepMetadata: 'Keep photo info (EXIF)',
    keepMetadataHint: 'Keeps capture time, camera model, GPS location, etc. Turn off for privacy; files also get a bit smaller. AVIF output cannot carry it yet.',
    keepIcc: 'Keep color profile (ICC)',
    keepIccHint: 'Keeps colors looking the same across screens — recommended on. Turning it off can shift colors in vivid photos. AVIF output cannot carry it yet.',
    themeDark: 'Dark',
  },
  confirm: { quitProcessing: '{count} files still processing. Quit anyway?' },
  alert: { engineCrashed: 'Image engine crashed. It will restart on the next drop.' },
};

export default messages;
