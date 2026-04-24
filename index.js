#!/usr/bin/env node
import { stat, readdir } from 'node:fs/promises';
import { resolve, join, extname, relative } from 'node:path';

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

async function main() {
  const { input, output } = parseArgs(process.argv);
  await validateInputDir(input);
  console.log(`Input:  ${input}`);
  console.log(`Output: ${output}`);
  const files = await discoverImages(input);
  console.log(`Found ${files.length} images`);
  for (const f of files) console.log(`  ${f.relPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
