//! Write the production AcceptedEvent JSON Schema into packages/schemas.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use cryptex_events::schema::{
    CHECKED_IN_SCHEMA_REL_PATH, CHECKED_IN_V2_SCHEMA_REL_PATH, render_accepted_event_schema_json,
    render_v2_knowledge_assertion_schema_json,
};

fn main() {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    root.pop();
    root.pop();
    for (rel_path, json) in [
        (
            CHECKED_IN_SCHEMA_REL_PATH,
            render_accepted_event_schema_json(),
        ),
        (
            CHECKED_IN_V2_SCHEMA_REL_PATH,
            render_v2_knowledge_assertion_schema_json(),
        ),
    ] {
        let out = root.join(rel_path);
        if let Some(parent) = out.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!("cryptex-events-schema-gen: create dir: {error}");
            process::exit(2);
        }
        if let Err(error) = fs::write(&out, json) {
            eprintln!(
                "cryptex-events-schema-gen: write {}: {error}",
                out.display()
            );
            process::exit(2);
        }
        println!("wrote {}", out.display());
    }
    let _ = env::args().collect::<Vec<_>>();
}
