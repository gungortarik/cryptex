import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { compileFromFile } from "json-schema-to-typescript";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const schemaPath = join(root, "cryptex-event-v1.schema.json");
const outPath = join(root, "cryptex-event-v1.ts");
const check = process.argv.includes("--check");

const banner =
  "/* eslint-disable */\n" +
  "/**\n" +
  " * Generated from cryptex-event-v1.schema.json. Do not edit by hand.\n" +
  " * Regenerate: cargo run -p cryptex-events --bin cryptex-events-schema-gen\n" +
  " *             pnpm --filter @cryptex/schemas generate:ts\n" +
  " */\n\n";

const ts = banner + (await compileFromFile(schemaPath, {
  additionalProperties: false,
  bannerComment: "",
  style: { singleQuote: false, semi: true },
}));

const normalized = ts.replace(/\r\n/g, "\n");
if (check) {
  const existing = readFileSync(outPath, "utf8").replace(/\r\n/g, "\n");
  if (existing !== normalized) {
    console.error("TypeScript schema binding drift detected for cryptex-event-v1.ts");
    process.exit(1);
  }
  console.log("cryptex-event-v1.ts matches schema");
  process.exit(0);
}

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, normalized, "utf8");
console.log(`wrote ${outPath}`);
