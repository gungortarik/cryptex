import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { hostTripleFromRustc } from "../../apps/desktop/scripts/prepare-cryptexd-sidecar.mjs";
import { collectRustNotices } from "../../apps/desktop/scripts/rust-notices.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const host = hostTripleFromRustc(execFileSync("rustc", ["-vV"], { encoding: "utf8" }));
const inventory = collectRustNotices(root, host, host, ["cryptex-updater"]);
writeFileSync(process.argv[2], JSON.stringify(inventory, null, 2) + "\n", { flag: "wx" });
