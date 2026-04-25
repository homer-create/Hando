// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Messages } from './en';

const messages: Messages = {
  toolbar: { settings: '⚙ 設定', undo: '↺ 元に戻す' },
  dropzone: {
    prompt: '画像をここにドラッグ、または{link}',
    clickToAdd: 'クリックで追加',
    imagesFilter: '画像',
  },
  fileList: { empty: 'ファイルがありません。画像をウィンドウにドラッグしてください。' },
  statusbar: {
    progress: '{completed} / {total} 件 ({pct}%)',
    saved: '{amount} を節約しました ({count} 件)',
    trashHint: '元ファイルをごみ箱に移動しました',
    trashShow: '表示',
  },
  settings: {
    title: '設定',
    language: '言語',
    languageAuto: 'システムに従う',
    jpegQuality: 'JPEG 品質',
    pngQuality: 'PNG 品質',
    webpQuality: 'WebP 品質',
    avifQuality: 'AVIF 品質',
    emitWebp: 'WebP も同時に出力',
    emitAvif: 'AVIF も同時に出力',
    moveToTrash: '元ファイルをごみ箱に移動',
    done: '完了',
  },
  confirm: { quitProcessing: '{count} 件のファイルを処理中です。終了しますか？' },
  alert: { engineCrashed: '画像処理エンジンがクラッシュしました。次のドロップで再起動します。' },
};

export default messages;
