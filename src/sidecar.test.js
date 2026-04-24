import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, rm, stat, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import sharp from 'sharp';

const TMP = './fixtures/test-sidecar';
const SIDECAR = resolve('./src/sidecar.js');

before(async () => {
  await mkdir(TMP, { recursive: true });
  await sharp({
    create: { width: 400, height: 400, channels: 3, background: { r: 255, g: 100, b: 50 } },
  }).jpeg({ quality: 100 }).toFile(join(TMP, 'big.jpg'));
});

after(async () => {
  await new Promise((r) => setTimeout(r, 200));
  try {
    await rm(TMP, { recursive: true, force: true });
  } catch (e) {
    if (e.code === 'EBUSY' && process.platform === 'win32') {
      const { execSync } = await import('node:child_process');
      execSync(`cmd.exe /c rd /s /q "${TMP.replace(/\//g, '\\\\')}"`);
    } else throw e;
  }
});

function runSidecar(commands, { timeoutMs = 5000 } = {}) {
  return new Promise((resolveP, rejectP) => {
    const proc = spawn('node', [SIDECAR], { stdio: ['pipe', 'pipe', 'pipe'] });
    let out = '';
    let err = '';
    proc.stdout.on('data', (d) => (out += d.toString()));
    proc.stderr.on('data', (d) => (err += d.toString()));
    const timer = setTimeout(() => {
      proc.kill('SIGKILL');
      rejectP(new Error(`sidecar timed out after ${timeoutMs}ms. stderr: ${err}`));
    }, timeoutMs);
    proc.on('close', (code) => {
      clearTimeout(timer);
      resolveP({ code, out, err });
    });
    for (const cmd of commands) proc.stdin.write(JSON.stringify(cmd) + '\n');
    proc.stdin.end();
  });
}

test('sidecar handles a single encode command and emits done', async () => {
  const id = 'test-1';
  const src = resolve(join(TMP, 'big.jpg'));
  const { out, code } = await runSidecar([
    { cmd: 'encode', id, src, ext: '.jpg', opts: { jpegQuality: 75 } },
  ]);
  assert.equal(code, 0);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const done = lines.find((l) => l.type === 'done' && l.id === id);
  assert.ok(done, `expected a done event, got: ${out}`);
  assert.ok(done.tmp && done.tmp.endsWith('.jpg'));
  assert.ok(done.srcBytes > 0);
  assert.ok(done.outBytes > 0);
  assert.ok(done.outBytes < done.srcBytes, 'compressed file should be smaller');
  assert.deepEqual(done.companions, []);
  await stat(done.tmp);
});

test('malformed JSON on stdin does not crash sidecar', async () => {
  const src = resolve(join(TMP, 'big.jpg'));
  const proc = spawn('node', [SIDECAR], { stdio: ['pipe', 'pipe', 'pipe'] });
  let out = '';
  proc.stdout.on('data', (d) => (out += d.toString()));
  proc.stdin.write('this-is-not-json\n');
  proc.stdin.write(JSON.stringify({ cmd: 'encode', id: 'after-bad', src, ext: '.jpg', opts: { jpegQuality: 75 } }) + '\n');
  proc.stdin.end();
  await new Promise((r) => proc.on('close', r));
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  assert.ok(lines.some((l) => l.type === 'parse-error'), 'expected parse-error event');
  assert.ok(lines.some((l) => l.type === 'done' && l.id === 'after-bad'), 'sidecar should continue after bad line');
});

test('encode on missing file emits error event', async () => {
  const { out } = await runSidecar([
    { cmd: 'encode', id: 'missing', src: '/no/such/file.jpg', ext: '.jpg', opts: { jpegQuality: 75 } },
  ]);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const err = lines.find((l) => l.type === 'error' && l.id === 'missing');
  assert.ok(err, `expected error event, got: ${out}`);
  assert.ok(err.msg && err.msg.length > 0);
});

test('no-gain skip: already-compressed tiny file', async () => {
  const tiny = join(TMP, 'tiny.jpg');
  await sharp({
    create: { width: 64, height: 64, channels: 3, background: { r: 10, g: 20, b: 30 } },
  }).jpeg({ quality: 40, mozjpeg: true }).toFile(tiny);
  const { out } = await runSidecar([
    { cmd: 'encode', id: 'tiny', src: resolve(tiny), ext: '.jpg', opts: { jpegQuality: 95 } },
  ]);
  const lines = out.trim().split('\n').map((l) => JSON.parse(l));
  const skip = lines.find((l) => l.type === 'skipped-no-gain' && l.id === 'tiny');
  assert.ok(skip, `expected skipped-no-gain, got: ${out}`);
  assert.ok(skip.srcBytes > 0);
});
