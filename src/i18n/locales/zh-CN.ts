// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Messages } from './en';

const messages: Messages = {
  toolbar: { settings: '⚙ 设置', undo: '↺ 撤销' },
  dropzone: {
    prompt: '拖动图片到此，或{link}',
    clickToAdd: '点击添加',
    imagesFilter: '图片',
  },
  fileList: { empty: '还没有文件。将图片拖入窗口。', skipped: '已优化，跳过' },
  statusbar: {
    progress: '{completed} / {total} 张 ({pct}%)',
    saved: '已节省 {amount}，共 {count} 个文件',
    trashHint: '原始文件已移至回收站',
    trashShow: '显示',
  },
  settings: {
    title: '设置',
    language: '语言',
    languageAuto: '跟随系统',
    jpegQuality: 'JPEG 质量',
    pngQuality: 'PNG 质量',
    webpQuality: 'WebP 质量',
    avifQuality: 'AVIF 质量',
    emitWebp: '同时输出 WebP',
    emitAvif: '同时输出 AVIF',
    moveToTrash: '将原始文件移至回收站',
    done: '完成',
    theme: '外观',
    themeAuto: '自动',
    themeLight: '浅色',
    mode: '模式',
    modeAuto: '自动（画质目标）',
    modeManual: '手动',
    preset: '画质目标',
    presetLossless: '视觉无损',
    presetBalanced: '平衡',
    presetAggressive: '激进',
    advanced: '高级',
    avifSpeed: 'AVIF 速度',
    oxipngLevel: 'PNG 压缩力度（oxipng）',
    webpMethod: 'WebP 压缩力度（method）',
    jpegProgressive: '渐进式 JPEG',
    themeDark: '深色',
  },
  confirm: { quitProcessing: '还有 {count} 个文件正在处理，要退出吗？' },
  alert: { engineCrashed: '图片处理引擎崩溃，下次拖动时会自动重启。' },
};

export default messages;
