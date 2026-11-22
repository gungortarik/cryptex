use cryptex_cortex_inventory::{list, preview};
use std::{env, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).take(4).collect();
    match args.as_slice() {
        [source] => list(Path::new(source)),
        [flag, source, item] if flag == "--preview-note" => {
            preview(Path::new(source), item.to_str())
        }
        _ => {
            eprintln!(
                "Usage: cryptex-cortex-inventory [--preview-note] SOURCE_DIRECTORY [INVENTORY_PATH]"
            );
            ExitCode::FAILURE
        }
    }
}
