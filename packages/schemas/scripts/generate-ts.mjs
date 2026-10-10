import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { compileFromFile } from "json-schema-to-typescript";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const check = process.argv.includes("--check");

for (const version of ["v1", "v2"]) {
  const schemaName = `cryptex-event-${version}.schema.json`;
  const schemaPath = join(root, schemaName);
  const outPath = join(root, `cryptex-event-${version}.ts`);
  const banner =
    "/* eslint-disable */\n" +
    "/**\n" +
    ` * Generated from ${schemaName}. Do not edit by hand.\n` +
    " * Regenerate: cargo run -p cryptex-events --bin cryptex-events-schema-gen\n" +
    " *             pnpm --filter @cryptex/schemas generate:ts\n" +
    " * Unknown-field rejection comes from schema additionalProperties:false.\n" +
    " */\n\n";

  // Unknown-field policy must come from each JSON Schema, not generator options.
  const ts = banner + (await compileFromFile(schemaPath, {
    bannerComment: "",
    style: { singleQuote: false, semi: true },
  }));
  const normalized = ts.replace(/\r\n/g, "\n");
  if (check) {
    const existing = readFileSync(outPath, "utf8").replace(/\r\n/g, "\n");
    if (existing !== normalized) {
      console.error(`TypeScript schema binding drift detected for ${outPath}`);
      process.exitCode = 1;
    } else {
      console.log(`${outPath} matches schema`);
    }
  } else {
    mkdirSync(dirname(outPath), { recursive: true });
    writeFileSync(outPath, normalized, "utf8");
    console.log(`wrote ${outPath}`);
  }
}
