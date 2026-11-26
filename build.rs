fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target = std::env::var("TARGET").expect("Cargo provides TARGET");
    if target.ends_with("-linux-android") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-z,max-page-size=16384");
    }
    if target.ends_with("-windows-gnullvm") || target.ends_with("-windows-gnu") {
        // MinGW otherwise attempts to export every symbol from the shared library.
        println!("cargo:rustc-cdylib-link-arg=-Wl,--exclude-all-symbols");
    }
}
