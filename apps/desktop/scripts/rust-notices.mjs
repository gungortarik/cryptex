import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";

function cargo(root, args) {
  return execFileSync("cargo", args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
}

function noticeDocument(directory, name, texts, expectedHash) {
  const file = path.resolve(directory, name);
  const relative = path.relative(directory, file);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) throw new Error(`External notice: ${name}`);
  const bytes = readFileSync(file);
  const sha256 = createHash("sha256").update(bytes).digest("hex");
  if (!bytes.length || (expectedHash && sha256 !== expectedHash)) throw new Error(`Invalid notice: ${name}`);
  const text = bytes.toString("utf8");
  texts.set(sha256, Buffer.from(text, "utf8").equals(bytes) ? { text } : { base64: bytes.toString("base64") });
  return { name: relative.split(path.sep).join("/"), sha256 };
}

export function collectRustNotices(root, triple, hostTriple = triple, products = ["cryptex-desktop", "cryptexd"]) {
  const features = products.includes("cryptex-desktop") ? ["--features", "tauri/custom-protocol"] : [];
  const filter = triple === hostTriple ? ["--filter-platform", triple] : [];
  const metadata = JSON.parse(cargo(root, ["metadata", "--locked", "--format-version", "1", ...filter, ...features]));
  const tree = cargo(root, ["tree", "--locked", "--target", triple, ...products.flatMap(name => ["-p", name]),
    ...features, "--edges", "normal,build", "--prefix", "none", "--format", "{p}"]);
  const packages = new Map();
  for (const pkg of metadata.packages) {
    const key = `${pkg.name} v${pkg.version}`;
    if (packages.has(key)) throw new Error(`Ambiguous Cargo source: ${key}`);
    packages.set(key, pkg);
  }

  const supplemental = path.resolve(root, "third_party/rust-notices");
  const sources = JSON.parse(readFileSync(path.join(supplemental, "sources.json"), "utf8"));
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
    const files = new Set(readdirSync(dir, { recursive: true, withFileTypes: true })
      .filter(entry => entry.isFile() && /^(licen[cs]e|copying|copyright|notice|unlicense)([._-]|$)/i.test(entry.name))
      .map(entry => path.relative(dir, path.join(entry.parentPath, entry.name)).split(path.sep).join("/")));
    if (pkg.license_file) files.add(pkg.license_file);
    const documents = [...files].sort().map(name => noticeDocument(dir, name, texts));
    for (const entry of sources[`${pkg.name}-${pkg.version}`] ?? []) {
      documents.push({ ...noticeDocument(supplemental, entry.file, texts, entry.sha256), source: entry.source });
    }
    if (!documents.length) throw new Error(`Missing licensed source notice for ${key}`);
    notices.push({ name: pkg.name, version: pkg.version, license: pkg.license, repository: pkg.repository, documents });
  }
  return { target: triple, scope: "normal and build dependencies",
    packages: notices, documents: Object.fromEntries(texts) };
}

export function writeRustNotices(root, triple, hostTriple, file) {
  writeFileSync(file, JSON.stringify(collectRustNotices(root, triple, hostTriple), null, 2) + "\n", "utf8");
}
