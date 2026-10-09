// The 32-bit build exports DirectInput's entry points by their plain names (a .def file: stdcall names are
// decorated otherwise), so the game finds them in our `dinput8.dll`.
fn main() {
    println!("cargo:rerun-if-changed=exports.def");
    if std::env::var("TARGET").is_ok_and(|t| t.starts_with("i686-pc-windows")) {
        let def = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("exports.def");
        println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def.display());
    }
}
