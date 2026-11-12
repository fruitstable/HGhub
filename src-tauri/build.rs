fn main() {
    println!("cargo::rustc-check-cfg=cfg(hghub_web)");
    tauri_build::build()
}
