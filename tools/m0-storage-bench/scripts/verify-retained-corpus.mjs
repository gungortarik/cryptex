#!/usr/bin/env node

/**
 * Verify a retained P0-1 benchmark corpus has not been altered.
 * Read-only. Refuses non-Cryptex-Dev benchmark paths.
 */

import fsp from "node:fs/promises";
import path from "node:path";

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (!arg.startsWith("--")) fail(`unexpected argument: ${arg}`);
    if (i + 1 >= argv.length) fail(`${arg} requires a value`);
    values.set(arg, argv[i + 1]);
    i += 1;
  }
  const corpus = values.get("--corpus");
  const expectedFiles = Number(values.get("--expected-files"));
  const expectedBytes = Number(values.get("--expected-bytes"));
  if (!corpus || !Number.isSafeInteger(expectedFiles) || !Number.isSafeInteger(expectedBytes)) {
    fail(
      "usage: verify-retained-corpus.mjs --corpus <path> --expected-files <n> --expected-bytes <n>",
    );
  }
  return {
    corpus: path.resolve(corpus),
    expectedFiles,
    expectedBytes,
  };
}

function normalizeForGuard(value) {
  return path.resolve(value).replaceAll("\\", "/");
}

function assertCorpusGuard(corpus) {
  const normalized = normalizeForGuard(corpus);
  if (!normalized.includes("/Cryptex-Dev/benchmarks/p0-1/")) {
    fail(`refusing non-retained P0-1 corpus path: ${corpus}`);
  }
}

async function readTreeStats(root) {
  let files = 0;
  let bytes = 0;
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    const entries = await fsp.readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.name === ".stfolder") continue;
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) stack.push(full);
      else if (entry.isFile()) {
        const stat = await fsp.stat(full);
        files += 1;
        bytes += stat.size;
      } else {
        fail(`unsupported entry: ${full}`);
      }
    }
  }
  return { files, bytes };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  assertCorpusGuard(args.corpus);
  const stats = await readTreeStats(args.corpus);
  if (stats.files !== args.expectedFiles || stats.bytes !== args.expectedBytes) {
    fail(
      `corpus mismatch: expected files=${args.expectedFiles} bytes=${args.expectedBytes}, observed files=${stats.files} bytes=${stats.bytes}`,
    );
  }
  console.log(`corpus=${args.corpus}`);
  console.log(`files=${stats.files}`);
  console.log(`bytes=${stats.bytes}`);
  console.log("verified=true");
  console.log("scope=retained_p0_1_corpus_verification_only");
}

main().catch((error) => {
  console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 2;
});
