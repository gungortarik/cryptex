import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";

function cargo(root, args) {
  return execFileSync("cargo", args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

export function collectRustNotices(root, triple, hostTriple = triple) {
  const filter = triple === hostTriple ? ["--filter-platform", triple] : [];
  const metadata = JSON.parse(cargo(root, ["metadata", "--locked", "--format-version", "1", ...filter, "--features", "tauri/custom-protocol"]));
  const tree = cargo(root, ["tree", "--locked", "--target", triple, "-p", "cryptex-desktop", "-p", "cryptexd",
    "--features", "tauri/custom-protocol", "--edges", "normal,build", "--prefix", "none", "--format", "{p}"]);
  const packages = new Map();
  for (const pkg of metadata.packages) {
    const key = `${pkg.name} v${pkg.version}`;
    if (packages.has(key)) throw new Error(`Ambiguous Cargo source: ${key}`);
    packages.set(key, pkg);
  }

  const notices = [];
  const texts = new Map();
  const selected = new Set();
  for (const line of tree.trim().split(/\r?\n/)) {
    if (!line.trim()) continue;
    const match = /^([^ ]+ v[^ ]+)(?: |$)/.exec(line);
    if (!match || !packages.has(match[1])) throw new Error(`Cannot resolve Cargo package in release graph: ${line}`);
    selected.add(match[1]);
  }
  for (const key of [...selected].sort()) {
    const pkg = packages.get(key);
    if (metadata.workspace_members.includes(pkg.id)) continue;
    if (!pkg.source?.startsWith("registry+") || !(pkg.license || pkg.license_file)) {
      throw new Error(`Review source or license for ${key}`);
    }
    const dir = path.dirname(pkg.manifest_path);
    const files = new Set(readdirSync(dir, { withFileTypes: true })
      .filter(entry => entry.isFile() && /^(licen[cs]e|copying|copyright|notice|unlicense)([._-]|$)/i.test(entry.name))
      .map(entry => entry.name));
    if (pkg.license_file) files.add(pkg.license_file);
    const documents = [];
    for (const name of [...files].sort()) {
      const absolute = path.resolve(dir, name);
      const relative = path.relative(dir, absolute);
      if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) throw new Error(`External license for ${key}`);
      const bytes = readFileSync(absolute);
      if (!bytes.length) throw new Error(`Empty notice for ${key}`);
      const hash = createHash("sha256").update(bytes).digest("hex");
      // Preserve exact upstream bytes, including CRLF and non-UTF-8 notices.
      const text = bytes.toString("utf8");
      texts.set(hash, Buffer.from(text, "utf8").equals(bytes) ? { text } : { base64: bytes.toString("base64") });
      documents.push({ name: relative.split(path.sep).join("/"), sha256: hash });
    }
    notices.push({ name: pkg.name, version: pkg.version, license: pkg.license, repository: pkg.repository, documents });
  }
  return { target: triple, scope: "normal and build dependencies",
    packages: notices, documents: Object.fromEntries(texts) };
}

export function writeRustNotices(root, triple, hostTriple, file) {
  writeFileSync(file, JSON.stringify(collectRustNotices(root, triple, hostTriple), null, 2) + "\n", "utf8");
}
