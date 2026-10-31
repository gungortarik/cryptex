#!/usr/bin/env node

/**
 * Emit the remaining P0-1 initial Syncthing matrix as unique folder IDs and
 * guarded working paths. Does not mutate Syncthing state.
 */

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
  const runTag = values.get("--run-tag");
  if (!runTag || !/^[a-z0-9]+$/.test(runTag)) {
    fail("usage: plan-syncthing-p0-1-matrix.mjs --run-tag <lowercase-alnum-nonce>");
  }
  return { runTag };
}

const SCALES = [
  { events: 10000, label: "10k", expectedFilesFiles: 10000, expectedBytes: 10360000, expectedFilesSegments: 10 },
  { events: 100000, label: "100k", expectedFilesFiles: 100000, expectedBytes: 103600000, expectedFilesSegments: 100 },
  { events: 500000, label: "500k", expectedFilesFiles: 500000, expectedBytes: 518000000, expectedFilesSegments: 500 },
];

const LAYOUTS = ["files", "segments"];
const DIRECTIONS = [
  { source: "macos", destination: "windows" },
  { source: "windows", destination: "macos" },
];

// Consumed / invalid for initial timing; must never be reissued.
const RETIRED_FOLDER_IDS = new Set([
  "p0-1-10k-files-macos-to-windows",
  "p0-1-10k-files-macos-to-windows-r2",
]);

function main() {
  const args = parseArgs(process.argv.slice(2));
  console.log(`run_tag=${args.runTag}`);
  console.log("scope=p0_1_initial_sync_matrix_plan_only");
  console.log("note=each_cell_requires_fresh_paths_and_must_not_reuse_consumed_folder_ids");

  for (const scale of SCALES) {
    for (const layout of LAYOUTS) {
      for (const direction of DIRECTIONS) {
        const folderId = `p0-1-${scale.label}-${layout}-${direction.source}-to-${direction.destination}-${args.runTag}`;
        if (RETIRED_FOLDER_IDS.has(folderId)) {
          fail(`planned folder id collides with retired id: ${folderId}`);
        }
        const expectedFiles =
          layout === "files" ? scale.expectedFilesFiles : scale.expectedFilesSegments;
        console.log("---");
        console.log(`folder_id=${folderId}`);
        console.log(`events=${scale.events}`);
        console.log(`layout=${layout}`);
        console.log(`source_platform=${direction.source}`);
        console.log(`destination_platform=${direction.destination}`);
        console.log(`expected_files=${expectedFiles}`);
        console.log(`expected_bytes=${scale.expectedBytes}`);
        console.log(
          `macos_work_path=$HOME/Library/Application Support/Cryptex-Dev/syncthing-p0-1-runs/${folderId}`,
        );
        console.log(
          `windows_work_path=%LOCALAPPDATA%\\Cryptex-Dev\\syncthing-p0-1-runs\\${folderId}`,
        );
      }
    }
  }
}

try {
  main();
} catch (error) {
  console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 2;
}
