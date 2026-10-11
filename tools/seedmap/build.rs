fn main() {
    println!("cargo:rustc-cfg=seedmap_core");
    println!("cargo:rerun-if-changed=build.rs");
}
