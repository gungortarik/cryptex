import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const inputs = ["Cargo.lock", "pnpm-lock.yaml", "rust-toolchain.toml", ".node-version", "package.json"];

export function sourceRevision(root) {
  const options = { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] };
  // Let Git establish root membership. Windows may spell the same directory with a short name.
  const checkout = spawnSync("git", ["rev-parse", "--show-prefix"], options);
  if (checkout.status !== 0 || checkout.stdout.trimEnd() !== "") {
    return { commit: null, clean: false };
  }
  const head = spawnSync("git", ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"], options);
  if (head.status === 1) return { commit: null, clean: false };
  if (head.status !== 0) throw new Error("Cannot inspect package source revision");
  const commit = head.stdout.trim();
  const changes = execFileSync("git", ["status", "--porcelain=v1", "--untracked-files=normal"], options);
  return { commit, clean: changes.length === 0 };
}

export function writeBuildProvenance(root, target, destination) {
  const sha256 = Object.fromEntries(inputs.map(name => [
    name, createHash("sha256").update(readFileSync(path.join(root, name))).digest("hex"),
  ]));
  const record = {
    schema: 1, source: sourceRevision(root), target, inputs_sha256: sha256,
    node: process.version, rustc: execFileSync("rustc", ["--version"], { cwd: root, encoding: "utf8" }).trim(),
  };
  writeFileSync(destination, JSON.stringify(record, null, 2) + "\n");
}
