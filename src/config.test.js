import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CONFIG } from './config.js';

test('CONFIG exposes quality defaults and extension list', () => {
  assert.equal(CONFIG.JPEG_QUALITY, 75);
  assert.equal(CONFIG.PNG_QUALITY, 75);
  assert.equal(CONFIG.WEBP_QUALITY, 75);
  assert.equal(CONFIG.CONCURRENCY, 4);
  assert.deepEqual(CONFIG.EXTENSIONS, ['.jpg', '.jpeg', '.png', '.webp']);
});
