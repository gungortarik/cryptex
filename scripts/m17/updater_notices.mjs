import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { hostTripleFromRustc } from "../../apps/desktop/scripts/prepare-cryptexd-sidecar.mjs";
import { collectRustNotices } from "../../apps/desktop/scripts/rust-notices.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const host = hostTripleFromRustc(execFileSync("rustc", ["-vV"], { encoding: "utf8" }));
const profile = process.argv[3];
if (!["default", "owner-transport-update"].includes(profile)) throw new Error("Unknown updater build");
const inventory = collectRustNotices(root, host, host, ["cryptex-updater"],
  profile === "default" ? [] : ["cryptex-updater/owner-transport-update"]);
writeFileSync(process.argv[2], JSON.stringify(inventory, null, 2) + "\n", { flag: "wx" });
