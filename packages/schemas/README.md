# @cryptex/schemas

Generated JSON Schema and TypeScript bindings for Cryptex events. V1 remains the
frozen ADR-0016 contract; v2 is the separate ADR-0017 typed reader schema.

## Regenerate

```bash
cargo run -p cryptex-events --bin cryptex-events-schema-gen
pnpm --filter @cryptex/schemas install
pnpm --filter @cryptex/schemas generate:ts
```

## Drift checks

```bash
cargo test -p cryptex-events schema
pnpm --filter @cryptex/schemas check:drift
```

Rust domain types in `crates/events` are the source of truth. Do not hand-edit generated artifacts.
