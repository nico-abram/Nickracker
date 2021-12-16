use std::env;
use std::path::PathBuf;

// See also tesseract_init_stub.cpp and ocr.rs
fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Can't read CARGO_MANIFEST_DIR env var"),
    );
    let src_dir = manifest_dir.join("src");
    let file_path = src_dir.join("tesseract_init_stub.cpp");

    println!("cargo:rerun-if-changed=src/tesseract_init_stub.cpp");

    let include_paths = find_tesseract_system_lib();

    let mut cc = cc::Build::new();
    cc.cpp(true)
        .flag("-EHsc")
        //.flag("-std:c++latest")
        .pic(false);

    for include_path in include_paths {
        cc.include(include_path);
    }
    cc.file(file_path);
    cc.compile("tesseract_init_stub");
}

// From https://github.com/ccouzens/tesseract-sys/blob/main/build.rs
fn find_tesseract_system_lib() -> Vec<String> {
    println!("cargo:rerun-if-env-changed=TESSERACT_INCLUDE_PATHS");
    println!("cargo:rerun-if-env-changed=TESSERACT_LINK_PATHS");
    println!("cargo:rerun-if-env-changed=TESSERACT_LINK_LIBS");

    let vcpkg = || {
        let lib = vcpkg::Config::new().find_package("tesseract").unwrap();

        vec![lib
            .include_paths
            .iter()
            .map(|x| x.to_string_lossy())
            .collect::<String>()]
    };

    let include_paths = env::var("TESSERACT_INCLUDE_PATHS").ok();
    let include_paths = include_paths.as_deref().map(|x| x.split(','));
    let link_paths = env::var("TESSERACT_LINK_PATHS").ok();
    let link_paths = link_paths.as_deref().map(|x| x.split(','));
    let link_libs = env::var("TESSERACT_LINK_LIBS").ok();
    let link_libs = link_libs.as_deref().map(|x| x.split(','));
    if let (Some(include_paths), Some(link_paths), Some(link_libs)) =
        (include_paths, link_paths, link_libs)
    {
        for link_path in link_paths {
            println!("cargo:rustc-link-search={}", link_path)
        }

        for link_lib in link_libs {
            println!("cargo:rustc-link-lib={}", link_lib)
        }

        include_paths.map(|x| x.to_string()).collect::<Vec<_>>()
    } else {
        vcpkg()
    }
}
