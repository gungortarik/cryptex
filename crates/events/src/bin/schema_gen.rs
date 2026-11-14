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
    let args = env::args().skip(1).collect::<Vec<_>>();
    let destination = match args.as_slice() {
        [] => root.join("packages/schemas"),
        [flag, directory] if flag == "--output-dir" => PathBuf::from(directory),
        _ => { eprintln!("usage: cryptex-events-schema-gen [--output-dir DIRECTORY]"); process::exit(2); }
    };
    for (rel_path, json) in [
        (
            CHECKED_IN_SCHEMA_REL_PATH,
            render_accepted_event_schema_json(),
        ),
        (
            CHECKED_IN_V2_SCHEMA_REL_PATH,
            render_v2_knowledge_assertion_schema_json(),
        ),
        ("packages/schemas/cryptex-bootstrap-v1.schema.json", cryptex_events::schema::m8::bootstrap_schema_json()),
        ("packages/schemas/cryptex-bootstrap-transcript-v1.schema.json", cryptex_events::schema::m8::transcript_schema_json()),
        ("packages/schemas/cryptex-event-v3.schema.json", cryptex_events::schema::m8::security_schema_json()),
    ] {
        let out = destination.join(PathBuf::from(rel_path).file_name().expect("schema filename"));
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
}
