import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, rm, stat } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { join } from 'node:path';
import sharp from 'sharp';
import { ENCODERS, encode } from './encoder.js';

const execFileAsync = promisify(execFile);

const TMP = './fixtures/test-encoder';

before(async () => {
  await mkdir(TMP, { recursive: true });
  await sharp({
    create: { width: 200, height: 200, channels: 3, background: { r: 255, g: 0, b: 0 } },
  }).jpeg().toFile(join(TMP, 'red.jpg'));
  await sharp({
    create: { width: 200, height: 200, channels: 3, background: { r: 0, g: 0, b: 255 } },
  }).png().toFile(join(TMP, 'blue.png'));
  await sharp({
    create: { width: 200, height: 200, channels: 3, background: { r: 0, g: 255, b: 0 } },
  }).webp().toFile(join(TMP, 'green.webp'));
});

after(async () => {
  // On Windows, Sharp's native addon holds file handles until GC after
  // toFile() / metadata() resolve. Use the shell-level rd command as it
  // can schedule deletion even for files with open handles.
  await new Promise((r) => setTimeout(r, 200));
  try {
    await rm(TMP, { recursive: true, force: true });
  } catch (e) {
    if (e.code === 'EBUSY' && process.platform === 'win32') {
      await execFileAsync('cmd.exe', ['/c', 'rd', '/s', '/q', TMP.replace(/\//g, '\\')]);
    } else {
      throw e;
    }
  }
});

test('ENCODERS has entries for jpg/jpeg/png/webp', () => {
  assert.ok(ENCODERS['.jpg']);
  assert.ok(ENCODERS['.jpeg']);
  assert.ok(ENCODERS['.png']);
  assert.ok(ENCODERS['.webp']);
});

test('encode produces a valid JPEG', async () => {
  const dst = join(TMP, 'red.out.jpg');
  const { outBytes } = await encode({
    srcPath: join(TMP, 'red.jpg'),
    dstPath: dst,
    ext: '.jpg',
    opts: { jpegQuality: 75 },
  });
  assert.ok(outBytes > 0, 'outBytes should be positive');
  const meta = await sharp(dst).metadata();
  assert.equal(meta.format, 'jpeg');
});

test('encode produces a valid palette PNG', async () => {
  const dst = join(TMP, 'blue.out.png');
  await encode({
    srcPath: join(TMP, 'blue.png'),
    dstPath: dst,
    ext: '.png',
    opts: { pngQuality: 75 },
  });
  const meta = await sharp(dst).metadata();
  assert.equal(meta.format, 'png');
});

test('encode produces a valid WebP', async () => {
  const dst = join(TMP, 'green.out.webp');
  await encode({
    srcPath: join(TMP, 'green.webp'),
    dstPath: dst,
    ext: '.webp',
    opts: { webpQuality: 75 },
  });
  const meta = await sharp(dst).metadata();
  assert.equal(meta.format, 'webp');
});

test('encode throws on unsupported extension', async () => {
  await assert.rejects(
    encode({ srcPath: join(TMP, 'red.jpg'), dstPath: join(TMP, 'x.tiff'), ext: '.tiff', opts: {} }),
    /Unsupported extension: \.tiff/,
  );
});

test('encode lowercases extension before lookup', async () => {
  const dst = join(TMP, 'red.upper.jpg');
  const { outBytes } = await encode({
    srcPath: join(TMP, 'red.jpg'),
    dstPath: dst,
    ext: '.JPG',
    opts: { jpegQuality: 75 },
  });
  assert.ok(outBytes > 0);
});
