#!/usr/bin/env node
import { stat, readdir, mkdir } from 'node:fs/promises';
import { resolve, join, extname, relative, dirname } from 'node:path';
import sharp from 'sharp';

const CONFIG = {
  JPEG_QUALITY: 75,
  PNG_QUALITY: 75,
  WEBP_QUALITY: 75,
  EXTENSIONS: ['.jpg', '.jpeg', '.png', '.webp'],
  CONCURRENCY: 4,
};

function printUsageAndExit() {
  console.error('Usage: imageopt <input-dir> -o <output-dir>');
  process.exit(1);
}

function parseArgs(argv) {
  const args = argv.slice(2);
  let input = null;
  let output = null;
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === '-o' || a === '--output') {
      output = args[++i];
    } else if (!a.startsWith('-') && input === null) {
      input = a;
    } else {
      console.error(`Unknown argument: ${a}`);
      printUsageAndExit();
    }
  }
  if (!input || !output) printUsageAndExit();
  return { input: resolve(input), output: resolve(output) };
}

async function validateInputDir(dir) {
  try {
    const s = await stat(dir);
    if (!s.isDirectory()) {
      console.error(`Input is not a directory: ${dir}`);
      process.exit(1);
    }
  } catch {
    console.error(`Input directory does not exist: ${dir}`);
    process.exit(1);
  }
}

async function discoverImages(rootDir) {
  const results = [];
  async function walk(dir) {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const entry of entries) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        await walk(full);
      } else if (entry.isFile()) {
        const ext = extname(entry.name).toLowerCase();
        if (CONFIG.EXTENSIONS.includes(ext)) {
          results.push({
            absPath: full,
            relPath: relative(rootDir, full),
            ext,
          });
        }
      }
    }
  }
  await walk(rootDir);
  return results;
}

async function needsRebuild(srcPath, outPath) {
  let srcStat;
  try {
    srcStat = await stat(srcPath);
  } catch {
    return false;
  }
  let outStat;
  try {
    outStat = await stat(outPath);
  } catch {
    return true;
  }
  return srcStat.mtimeMs > outStat.mtimeMs;
}

function encodeForExt(pipeline, ext) {
  switch (ext) {
    case '.jpg':
    case '.jpeg':
      return pipeline.jpeg({ quality: CONFIG.JPEG_QUALITY, mozjpeg: true });
    case '.png':
      return pipeline.png({ quality: CONFIG.PNG_QUALITY, palette: true, compressionLevel: 9 });
    case '.webp':
      return pipeline.webp({ quality: CONFIG.WEBP_QUALITY });
    default:
      throw new Error(`Unsupported extension: ${ext}`);
  }
}

async function writeIfNeeded(srcPath, outPath, encodeFn) {
  if (!(await needsRebuild(srcPath, outPath))) {
    return { wrote: false, srcBytes: 0, outBytes: 0 };
  }
  await mkdir(dirname(outPath), { recursive: true });
  const pipeline = encodeFn(sharp(srcPath));
  const { size: outBytes } = await pipeline.toFile(outPath);
  const { size: srcBytes } = await stat(srcPath);
  return { wrote: true, srcBytes, outBytes };
}

async function processFile(file, outputRoot) {
  const outSameFormat = join(outputRoot, file.relPath);
  const outWebp = join(
    outputRoot,
    file.relPath.replace(new RegExp(`${file.ext.replace('.', '\\.')}$`, 'i'), '.webp'),
  );

  const results = [];
  if (file.ext !== '.webp') {
    results.push(
      await writeIfNeeded(file.absPath, outSameFormat, (p) => encodeForExt(p, file.ext)),
    );
  }
  results.push(
    await writeIfNeeded(file.absPath, outWebp, (p) => encodeForExt(p, '.webp')),
  );
  return results;
}

async function runPool(items, limit, worker) {
  const results = new Array(items.length);
  let next = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (true) {
      const i = next++;
      if (i >= items.length) return;
      results[i] = await worker(items[i], i);
    }
  });
  await Promise.all(workers);
  return results;
}

async function main() {
  const { input, output } = parseArgs(process.argv);
  await validateInputDir(input);
  console.log(`Input:  ${input}`);
  console.log(`Output: ${output}`);
  const files = await discoverImages(input);
  console.log(`Found ${files.length} images`);
  let processed = 0, skipped = 0, failed = 0, srcTotal = 0, outTotal = 0;
  const startedAt = Date.now();
  const perFile = await runPool(files, CONFIG.CONCURRENCY, async (file) => {
    try {
      const results = await processFile(file, output);
      return { ok: true, results };
    } catch (err) {
      console.warn(`WARN: failed ${file.relPath}: ${err.message}`);
      return { ok: false };
    }
  });
  for (const item of perFile) {
    if (!item.ok) { failed++; continue; }
    for (const r of item.results) {
      if (r.wrote) { processed++; srcTotal += r.srcBytes; outTotal += r.outBytes; }
      else skipped++;
    }
  }
  const seconds = ((Date.now() - startedAt) / 1000).toFixed(2);
  console.log(`Processed: ${processed}   Skipped: ${skipped}   Failed: ${failed}`);
  console.log(`Time: ${seconds}s${srcTotal > 0 ? `     Saved: ${((srcTotal - outTotal) / 1024).toFixed(1)} KB` : ''}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
