// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
import sharp from 'sharp';

export const ENCODERS = {
  '.jpg':  (pipeline, opts) => pipeline.jpeg({ quality: opts.jpegQuality, mozjpeg: true }),
  '.jpeg': (pipeline, opts) => pipeline.jpeg({ quality: opts.jpegQuality, mozjpeg: true }),
  '.png':  (pipeline, opts) => pipeline.png({ quality: opts.pngQuality, palette: true, compressionLevel: 9 }),
  '.webp': (pipeline, opts) => pipeline.webp({ quality: opts.webpQuality }),
  '.avif': (pipeline, opts) => pipeline.avif({ quality: opts.avifQuality }),
};

export async function encode({ srcPath, dstPath, ext, opts }) {
  const key = ext.toLowerCase();
  const encoder = ENCODERS[key];
  if (!encoder) throw new Error(`Unsupported extension: ${ext}`);
  const pipeline = encoder(sharp(srcPath), opts);
  const { size: outBytes } = await pipeline.toFile(dstPath);
  return { outBytes };
}
