// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Messages } from './en';

const messages: Messages = {
  toolbar: { settings: '⚙ Configurações', undo: '↺ Desfazer' },
  dropzone: {
    prompt: 'Arraste imagens aqui ou {link}',
    clickToAdd: 'clique para adicionar',
    imagesFilter: 'Imagens',
  },
  fileList: { empty: 'Ainda sem arquivos. Arraste imagens para a janela.' },
  statusbar: {
    progress: '{completed} / {total} arquivos ({pct}%)',
    saved: '{amount} economizados em {count} arquivos',
    trashHint: 'Originais movidos para a Lixeira',
    trashShow: 'Mostrar',
  },
  settings: {
    title: 'Configurações',
    language: 'Idioma',
    languageAuto: 'Detecção automática (sistema)',
    jpegQuality: 'Qualidade JPEG',
    pngQuality: 'Qualidade PNG',
    webpQuality: 'Qualidade WebP',
    avifQuality: 'Qualidade AVIF',
    emitWebp: 'Também exportar WebP',
    emitAvif: 'Também exportar AVIF',
    moveToTrash: 'Mover originais para a Lixeira',
    done: 'Concluído',
    theme: 'Tema',
    themeAuto: 'Auto',
    themeLight: 'Claro',
    themeDark: 'Escuro',
  },
  confirm: { quitProcessing: '{count} arquivos ainda em processamento. Sair mesmo assim?' },
  alert: { engineCrashed: 'Mecanismo de imagens travou. Será reiniciado na próxima ação.' },
};

export default messages;
