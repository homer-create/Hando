// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Messages } from './en';

const messages: Messages = {
  toolbar: { settings: '⚙ Ajustes', undo: '↺ Deshacer' },
  dropzone: {
    prompt: 'Arrastra imágenes aquí, o {link}',
    clickToAdd: 'haz clic para añadir',
    imagesFilter: 'Imágenes',
  },
  fileList: { empty: 'Aún no hay archivos. Arrastra imágenes a la ventana.', skipped: 'Already optimized' },
  statusbar: {
    progress: '{completed} / {total} archivos ({pct}%)',
    saved: 'Ahorrados {amount} en {count} archivos',
    trashHint: 'Originales movidos a la Papelera',
    trashShow: 'Mostrar',
  },
  settings: {
    title: 'Ajustes',
    language: 'Idioma',
    languageAuto: 'Detección automática (sistema)',
    jpegQuality: 'Calidad JPEG',
    pngQuality: 'Calidad PNG',
    webpQuality: 'Calidad WebP',
    avifQuality: 'Calidad AVIF',
    emitWebp: 'También exportar WebP',
    emitAvif: 'También exportar AVIF',
    moveToTrash: 'Mover originales a la Papelera',
    done: 'Listo',
    theme: 'Tema',
    themeAuto: 'Auto',
    themeLight: 'Claro',
    mode: 'Modo',
    modeAuto: 'Automático (objetivo de calidad)',
    modeManual: 'Manual',
    preset: 'Objetivo de calidad',
    presetLossless: 'Visualmente sin pérdida',
    presetBalanced: 'Equilibrado',
    presetAggressive: 'Agresivo',
    advanced: 'Avanzado',
    avifSpeed: 'Velocidad AVIF',
    oxipngLevel: 'Esfuerzo PNG (oxipng)',
    webpMethod: 'Esfuerzo WebP (method)',
    jpegProgressive: 'JPEG progresivo',
    themeDark: 'Oscuro',
  },
  confirm: { quitProcessing: '{count} archivos aún se están procesando. ¿Salir de todos modos?' },
  alert: { engineCrashed: 'El motor de imágenes falló. Se reiniciará en la próxima acción.' },
};

export default messages;
