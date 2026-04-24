#!/usr/bin/env node
// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import { createInterface } from 'node:readline';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { stat, unlink } from 'node:fs/promises';
import { encode } from './encoder.js';
import { CONFIG } from './config.js';

const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });

function emit(event) {
  process.stdout.write(JSON.stringify(event) + '\n');
}

const CONCURRENCY = CONFIG.CONCURRENCY;
let inFlight = 0;
const queue = [];

function schedule(fn) {
  return new Promise((resolve) => {
    const job = async () => {
      inFlight++;
      try { await fn(); } finally {
        inFlight--;
        const next = queue.shift();
        if (next) next();
      }
      resolve();
    };
    if (inFlight < CONCURRENCY) job();
    else queue.push(job);
  });
}

async function handleEncode({ id, src, ext, opts }) {
  if (!/^\.[a-z0-9]+$/i.test(ext)) {
    emit({ type: 'error', id, msg: `Invalid extension: ${ext}` });
    return;
  }
  let mainTmp;
  try {
    mainTmp = join(tmpdir(), `hando-${randomUUID()}${ext}`);
    const { outBytes } = await encode({ srcPath: src, dstPath: mainTmp, ext, opts });
    const { size: srcBytes } = await stat(src);
    if (outBytes >= srcBytes) {
      await unlink(mainTmp).catch(() => {});
      emit({ type: 'skipped-no-gain', id, srcBytes });
      return;
    }
    const companions = [];
    if (opts.emitWebp && ext !== '.webp') {
      const compTmp = join(tmpdir(), `hando-${randomUUID()}.webp`);
      try {
        const { outBytes: compBytes } = await encode({
          srcPath: src, dstPath: compTmp, ext: '.webp', opts,
        });
        companions.push({ ext: '.webp', tmp: compTmp, outBytes: compBytes });
      } catch (err) {
        emit({ type: 'companion-error', id, ext: '.webp', msg: err.message });
      }
    }
    if (opts.emitAvif && ext !== '.avif') {
      const compTmp = join(tmpdir(), `hando-${randomUUID()}.avif`);
      try {
        const { outBytes: compBytes } = await encode({
          srcPath: src, dstPath: compTmp, ext: '.avif', opts,
        });
        companions.push({ ext: '.avif', tmp: compTmp, outBytes: compBytes });
      } catch (err) {
        emit({ type: 'companion-error', id, ext: '.avif', msg: err.message });
      }
    }
    emit({ type: 'done', id, tmp: mainTmp, srcBytes, outBytes, companions });
  } catch (err) {
    if (mainTmp) await unlink(mainTmp).catch(() => {});
    emit({ type: 'error', id, msg: err.message });
  }
}

const pending = [];

for await (const line of rl) {
  if (!line.trim()) continue;
  let msg;
  try {
    msg = JSON.parse(line);
  } catch (err) {
    emit({ type: 'parse-error', msg: err.message, line });
    continue;
  }
  if (msg.cmd === 'encode') pending.push(schedule(() => handleEncode(msg)));
  else emit({ type: 'error', id: msg.id ?? null, msg: `Unknown cmd: ${msg.cmd}` });
}

// Wait for all in-flight and queued jobs to complete before exiting
await Promise.all(pending);
