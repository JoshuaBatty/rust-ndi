#[allow(unused_imports)]
use std::io::ErrorKind;
use std::path::{PathBuf, Path};
use std::{env, fs};

#[cfg(target_os = "macos")]
fn main() {
    // Base path to the NDI SDK from the environment variable
    let ndi_sdk_path = env::var("NDI_SDK_DIR").expect("NDI_SDK_DIR environment variable not set");

    // Paths to the include and main header file
    let ndi_include_path = format!("{}/include", ndi_sdk_path);
    let main_header = format!("{}/Processing.NDI.Lib.h", ndi_include_path);

    // Path to the library directory
    let lib_path = format!("{}/lib/macOS", ndi_sdk_path);

    // Inform cargo about the search path for the linker and the library to link against
    println!("cargo:rustc-link-search=native={}", lib_path);
    println!("cargo:rustc-link-lib=dylib=ndi");

    // Set rpath
    println!("cargo:rustc-link-arg=-rpath");
    println!("cargo:rustc-link-arg={}", lib_path);

    // Generate the bindings
    let bindings = bindgen::Builder::default()
        .header(main_header)
        .clang_arg(format!("-I{}", ndi_include_path))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file
    let out_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR environment variable not set"));
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}


fn choose_source_dir() -> Option<String> {
    // Follow the 'recommended' install path
    if let Ok(path) = env::var("NDI_RUNTIME_DIR_V3") {
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

    // Try the local lib folder
    let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir).join("lib");
    if path.exists() {
        return path.to_str().map(|s| s.to_string());
    }

    None
}

#[cfg(target_os = "windows")]
fn main() {
    let source_dir = choose_source_dir();

    // Copy the .dll/.lib files to the deps folder, to make it build
    if let Some(path) = source_dir {
        let source_path = Path::new(&path);
        let dest_path = Path::new(&env::var("OUT_DIR").unwrap()).join("../../../deps");
        fs::copy(
            source_path.join("..\\..\\NewTek NDI 3.8 SDK\\Lib\\x64\\Processing.NDI.Lib.x64.lib"),
            dest_path.join("Processing.NDI.Lib.x64.lib"),
        )
        .expect("copy Processing.NDI.Lib.x64.lib");
        fs::copy(
            source_path.join("Processing.NDI.Lib.x64.dll"),
            dest_path.join("Processing.NDI.Lib.x64.dll"),
        )
        .expect("copy Processing.NDI.Lib.x64.dll");
    }

    if cfg!(not(feature = "dynamic-link")) {
        // Static link against it
        println!("cargo:rustc-link-lib=Processing.NDI.Lib.x64");
    }
}

#[cfg(target_os = "linux")]
fn main() {
    let source_dir = choose_source_dir();

    // Copy the .so files to the deps folder, to make it build
    if let Some(path) = source_dir {
        let source_path = Path::new(&path);
        let dest_path = Path::new(&env::var("OUT_DIR").unwrap()).join("../../../deps");
        fs::copy(source_path.join("libndi.so.3"), dest_path.join("libndi.so.3")).expect("copy libndi.so.3");

        let sl_res = std::os::unix::fs::symlink(Path::new("libndi.so.3"), dest_path.join("libndi.so"));
        if let Err(e) = sl_res {
            if e.kind() != ErrorKind::AlreadyExists {
                panic!("Unknown error: {}", e);
            }
        }
    }

    if cfg!(not(feature = "dynamic-link")) {
        // Static link against it
        println!("cargo:rustc-link-lib=ndi");
    }
}