#!/usr/bin/env node
import { createInterface } from 'node:readline';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { stat } from 'node:fs/promises';
import { encode } from './encoder.js';

const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });

function emit(event) {
  process.stdout.write(JSON.stringify(event) + '\n');
}

async function handleEncode({ id, src, ext, opts }) {
  const tmp = join(tmpdir(), `imageopt-${randomUUID()}${ext}`);
  const { outBytes } = await encode({ srcPath: src, dstPath: tmp, ext, opts });
  const { size: srcBytes } = await stat(src);
  emit({ type: 'done', id, tmp, srcBytes, outBytes, companions: [] });
}

for await (const line of rl) {
  if (!line.trim()) continue;
  const msg = JSON.parse(line);
  if (msg.cmd === 'encode') await handleEncode(msg);
}
