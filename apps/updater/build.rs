use std::{env, fs, path::PathBuf};

fn main() {
    const PIN: &str = "CRYPTEX_UPDATE_SIGNER_DER";
    println!("cargo:rerun-if-env-changed={PIN}");
    let certificate = env::var_os(PIN).map_or_else(Vec::new, |source| {
        let source = PathBuf::from(source);
        assert!(
            source.is_absolute(),
            "owner certificate must be an absolute path"
        );
        println!("cargo:rerun-if-changed={}", source.display());
        assert!(
            (1..=65536).contains(&fs::metadata(&source).unwrap().len()),
            "invalid public certificate length"
        );
        fs::read(source).expect("read owner public certificate")
    });
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("update-signer.der"),
        certificate,
    )
    .unwrap();

    let target = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let mut build = cc::Build::new();
    match target.as_str() {
        "windows" => {
            build.file("native/windows.c");
            println!("cargo:rustc-link-lib=wintrust");
            println!("cargo:rustc-link-lib=crypt32");
        }
        "macos" => {
            let verifier = "../../services/cryptexd/src/m8_owner_channel/native/inventory.c";
            println!("cargo:rerun-if-changed={verifier}");
            build.file("native/macos.c").file(verifier);
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
        }
        _ => return,
    }
    println!("cargo:rerun-if-changed=native");
    build
        .warnings_into_errors(true)
        .compile("cryptex_update_signatures");
}
