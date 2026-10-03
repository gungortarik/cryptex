import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const desktopDirectory = path.resolve(scriptDirectory, "..");
const repositoryDirectory = path.resolve(desktopDirectory, "..", "..");
const tauriDirectory = path.join(desktopDirectory, "src-tauri");

export function hostTripleFromRustc(versionOutput) {
  const match = versionOutput.match(/^host:\s*(\S+)$/m);
  if (!match) {
    throw new Error("rustc -vV did not report a host target triple");
  }
  return match[1];
}

export function sidecarFileName(targetTriple) {
  if (!/^[A-Za-z0-9_-]+$/.test(targetTriple)) {
    throw new Error(`invalid Rust target triple: ${targetTriple}`);
  }
  const extension = targetTriple.includes("windows") ? ".exe" : "";
  return `cryptexd-${targetTriple}${extension}`;
}

export function sidecarBuildPaths(repositoryRoot, targetTriple, targetDirectory) {
  const extension = targetTriple.includes("windows") ? ".exe" : "";
  const cargoTarget = targetDirectory ?? path.join(repositoryRoot, "target");
  return {
    builtBinary: path.join(cargoTarget, targetTriple, "release", `cryptexd${extension}`),
    stagedBinary: path.join(
      repositoryRoot,
      "apps",
      "desktop",
      "src-tauri",
      "binaries",
      sidecarFileName(targetTriple),
    ),
  };
}

export function prepareSidecar(targetTriple = process.env.TAURI_ENV_TARGET_TRIPLE) {
  const triple = targetTriple ?? hostTripleFromRustc(execFileSync("rustc", ["-vV"], { encoding: "utf8" }));
  const cargoTargetDirectory = process.env.CARGO_TARGET_DIR
    ? path.resolve(repositoryDirectory, process.env.CARGO_TARGET_DIR)
    : undefined;
  const paths = sidecarBuildPaths(repositoryDirectory, triple, cargoTargetDirectory);

  execFileSync(
    "cargo",
    ["build", "-p", "cryptexd", "--release", "--locked", "--target", triple],
    { cwd: repositoryDirectory, stdio: "inherit" },
  );

  mkdirSync(path.dirname(paths.stagedBinary), { recursive: true });
  copyFileSync(paths.builtBinary, paths.stagedBinary);
  if (!triple.includes("windows")) {
    chmodSync(paths.stagedBinary, 0o755);
  }
  process.stdout.write(`Prepared Tauri sidecar for ${triple}: ${paths.stagedBinary}\n`);
  return paths;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    prepareSidecar(process.argv[2]);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
