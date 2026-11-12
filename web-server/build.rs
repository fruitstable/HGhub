fn main() {
    println!("cargo::rustc-check-cfg=cfg(hghub_web)");
    println!("cargo::rustc-cfg=hghub_web");
}
