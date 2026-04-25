// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import type { Messages } from './en';

const messages: Messages = {
  toolbar: { settings: '⚙ 설정', undo: '↺ 실행 취소' },
  dropzone: {
    prompt: '이미지를 여기로 끌어다 놓거나 {link}',
    clickToAdd: '클릭하여 추가',
    imagesFilter: '이미지',
  },
  fileList: { empty: '파일이 없습니다. 이미지를 창으로 끌어다 놓으세요.' },
  statusbar: {
    progress: '{completed} / {total} 개 ({pct}%)',
    saved: '{amount} 절약, {count}개 파일',
    trashHint: '원본 파일을 휴지통으로 이동했습니다',
    trashShow: '보기',
  },
  settings: {
    title: '설정',
    language: '언어',
    languageAuto: '시스템 따르기',
    jpegQuality: 'JPEG 품질',
    pngQuality: 'PNG 품질',
    webpQuality: 'WebP 품질',
    avifQuality: 'AVIF 품질',
    emitWebp: 'WebP도 함께 내보내기',
    emitAvif: 'AVIF도 함께 내보내기',
    moveToTrash: '원본 파일을 휴지통으로 이동',
    done: '완료',
  },
  confirm: { quitProcessing: '{count}개 파일이 처리 중입니다. 종료하시겠습니까?' },
  alert: { engineCrashed: '이미지 엔진이 다운되었습니다. 다음 끌어다 놓을 때 재시작됩니다.' },
};

export default messages;
