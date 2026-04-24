#!/usr/bin/env node
import { createInterface } from 'node:readline';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { stat, unlink } from 'node:fs/promises';
import { encode } from './encoder.js';

const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });

function emit(event) {
  process.stdout.write(JSON.stringify(event) + '\n');
}

async function handleEncode({ id, src, ext, opts }) {
  if (!/^\.[a-z0-9]+$/i.test(ext)) {
    emit({ type: 'error', id, msg: `Invalid extension: ${ext}` });
    return;
  }
  let tmp;
  try {
    tmp = join(tmpdir(), `imageopt-${randomUUID()}${ext}`);
    const { outBytes } = await encode({ srcPath: src, dstPath: tmp, ext, opts });
    const { size: srcBytes } = await stat(src);
    if (outBytes >= srcBytes) {
      await unlink(tmp).catch(() => {});
      emit({ type: 'skipped-no-gain', id, srcBytes });
      return;
    }
    emit({ type: 'done', id, tmp, srcBytes, outBytes, companions: [] });
  } catch (err) {
    if (tmp) await unlink(tmp).catch(() => {});
    emit({ type: 'error', id, msg: err.message });
  }
}

for await (const line of rl) {
  if (!line.trim()) continue;
  let msg;
  try {
    msg = JSON.parse(line);
  } catch (err) {
    emit({ type: 'parse-error', msg: err.message, line });
    continue;
  }
  try {
    if (msg.cmd === 'encode') await handleEncode(msg);
    else emit({ type: 'error', id: msg.id ?? null, msg: `Unknown cmd: ${msg.cmd}` });
  } catch (err) {
    emit({ type: 'error', id: msg.id ?? null, msg: err.message });
  }
}
