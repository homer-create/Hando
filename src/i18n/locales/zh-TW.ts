// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Messages } from './en';

const messages: Messages = {
  toolbar: { settings: '⚙ 設定', undo: '↺ 復原' },
  dropzone: {
    prompt: '拖曳圖片到此，或{link}',
    clickToAdd: '點擊新增',
    imagesFilter: '圖片',
  },
  fileList: { empty: '還沒有檔案。將圖片拖入視窗。', skipped: '已優化，略過' },
  statusbar: {
    progress: '{completed} / {total} 張 ({pct}%)',
    saved: '已節省 {amount}，共 {count} 個檔案',
    trashHint: '原始檔已移至資源回收筒',
    trashShow: '顯示',
  },
  settings: {
    title: '設定',
    language: '語言',
    languageAuto: '跟隨系統',
    jpegQuality: 'JPEG 品質',
    pngQuality: 'PNG 品質',
    webpQuality: 'WebP 品質',
    avifQuality: 'AVIF 品質',
    emitWebp: '同時輸出 WebP',
    emitAvif: '同時輸出 AVIF',
    moveToTrash: '將原始檔移至資源回收筒',
    done: '完成',
    theme: '外觀',
    themeAuto: '自動',
    themeLight: '亮色',
    mode: '模式',
    modeAuto: '自動（畫質目標）',
    modeManual: '手動',
    preset: '畫質目標',
    presetLossless: '視覺無損',
    presetBalanced: '平衡',
    presetAggressive: '激進',
    advanced: '進階',
    avifSpeed: 'AVIF 速度',
    oxipngLevel: 'PNG 壓縮力度（oxipng）',
    webpMethod: 'WebP 壓縮力度（method）',
    jpegProgressive: '漸進式 JPEG',
    themeDark: '深色',
  },
  confirm: { quitProcessing: '還有 {count} 個檔案處理中，要結束嗎？' },
  alert: { engineCrashed: '圖片處理引擎當機，下次拖曳時會自動重啟。' },
};

export default messages;
