#!/usr/bin/env node
import { stat } from 'node:fs/promises';
import { resolve } from 'node:path';

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

async function main() {
  const { input, output } = parseArgs(process.argv);
  await validateInputDir(input);
  console.log(`Input:  ${input}`);
  console.log(`Output: ${output}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
