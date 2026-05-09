fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    println!("cargo:rustc-env=JUST_TALK_TARGET_OS={}", target_os);

    // macOS: link required frameworks for audio and accessibility
    if target_os == "macos" {
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=AudioToolbox");
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
    }
}
