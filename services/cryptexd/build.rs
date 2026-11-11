fn main() {
    println!("cargo:rerun-if-env-changed=CRYPTEX_M8_LOCAL_STARTUP_CERT_DER");
    if std::env::var_os("CARGO_FEATURE_M8_LOCAL_STARTUP_CHECK").is_some() {
        assert_eq!(
            std::env::var("CARGO_CFG_TARGET_OS").as_deref(),
            Ok("macos"),
            "local signed startup check requires macOS"
        );
        let source = std::env::var_os("CRYPTEX_M8_LOCAL_STARTUP_CERT_DER")
            .expect("local signed startup check requires an independent build-time DER pin");
        let source = std::path::PathBuf::from(source);
        assert!(
            source.is_absolute(),
            "certificate pin path must be absolute"
        );
        println!("cargo:rerun-if-changed={}", source.display());
        let mut file = std::fs::File::open(source).expect("cannot read build-time DER pin");
        let mut cert = Vec::new();
        use std::io::Read;
        (&mut file)
            .take(65537)
            .read_to_end(&mut cert)
            .expect("cannot read DER pin");
        assert!(
            !cert.is_empty() && cert.len() <= 65536,
            "invalid DER pin size"
        );
        let target = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("m8-local-signer.der");
        std::fs::write(&target, cert).expect("cannot copy DER pin");
        assert!(
            std::process::Command::new("/usr/bin/openssl")
                .args(["x509", "-inform", "DER", "-noout", "-in"])
                .arg(target)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("cannot validate DER pin")
                .success(),
            "invalid DER certificate pin"
        );
    }
    let native = "src/m8_owner_channel/native";
    println!("cargo:rerun-if-changed={native}/channel.c");
    println!("cargo:rerun-if-changed={native}/channel.h");
    println!("cargo:rerun-if-changed={native}/inventory.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file(format!("{native}/channel.c"))
            .file(format!("{native}/inventory.c"))
            .std("c11")
            .warnings_into_errors(true)
            .compile("cryptex_m8_native");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=bsm");
    }
}
