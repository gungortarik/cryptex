fn main() {
    let native = "src/m8_owner_channel/native";
    println!("cargo:rerun-if-changed={native}/channel.c");
    println!("cargo:rerun-if-changed={native}/channel.h");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file(format!("{native}/channel.c"))
            .std("c11")
            .warnings_into_errors(true)
            .compile("cryptex_m8_native");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=bsm");
    }
}
