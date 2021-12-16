## Building

Building the main (root) crate/program on windows requires [tesseract](), an OCR library. It also requires a nightly rust toolchain. I would recommend installing rust using [rustup](https://rustup.rs/). You can install nightly rust once you have rustup by running `rustup toolchain install nightly` .
I have also only built and tested this on windows. The main program definitely only works on windows, but the basic console program should work anywhere, and the library solver can be reused.
You can build and install tesseract using [vcpkg](https://vcpkg.io/en/index.html). First install vcpkg, and then run `.\vcpkg install tesseract:x64-windows-static-md tesseract:x64-windows-static leptonica:x64-windows-static-md leptonica:x64-windows-static ` to install it. A different command is needed for 32bit, see [here](https://github.com/houqp/leptess). You may also need to install [LLVM](https://llvm.org/) and set the LIBCLANG_PATH environment variable, see [here](https://github.com/houqp/leptess/issues/19).

I use lld-link.exe instead of the msvc/VS link.exe for linking, which is faster. This comes with LLVM, and requires you to add the llvm /bin folder to your PATH. If you don't want to do this and want to use link.exe, you should be able to just comment the lld-link line in the /.cargo/config file.

Please ignore the files `xlto_download_c_deps.bat`, `xlto_compile_c_and_rust.bat` and `xlto_cargo_run.bat`. Those are only needed if you want to build with cross language LTO enabled. If you want to try, be warned that you will probably get a ton of errors (Linker errors, cmake errors, C compiler errors, rust errors, etc). In theory, running the download script and then the compile script should work.

## Project structure

- /solver contains the mastermind (minotaur vault) solver. It is a "12 color 4 peg" mastermind problem. The tests can be run with `cargo test --release` in /solver or `cargo test --release -p solver` in the root. It has ignored-by-default tests that perform an exhaustive test of all possible answers. You can run these with `cargo test --release test::exhaustive -- --nocapture --ignored --exact` (There are also 5 tests for this same exhaustive test but split into fifhts. These are run in CI in 5 separate jobs so they run a bit faster).
- /console contains a simple barebones console application to run the solver
- /tessdata contains traineddata files for the tesseract OCR library from https://github.com/tesseract-ocr/tessdata_best and https://github.com/tesseract-ocr/tessdata_fast
- /dbg is an output folder for debugging images. If you enable debug image output, the main binary will generate _many_ images in this folder, for example fragments of the screenshot where it performs OCR or where it looks for subimages, or where it thinks the minotaur vault window is.
- The find-subimage crate/library I wrote for this program lives in a separate repository [here](https://github.com/nico-abram/find-subimage)
- build.rs is a build script to build and link the /src/tesseract_init_stub.cpp C++ file. tesseract does not expose an initializer that lets us use training data from memory in the C API, and needs a file. The C++ API however has the constructor we need, so I wrote a simple wrapper that lets us call it.
- /src has the code for the main program.

Not important:

- The xlto\_\* batch scripts are for building with cross language LTO. You can ignore them.
- The vcpkg.json file is a vcpkg manifest that specifies dependencies. It is only used in CI.
- rust-toolchain makes cargo use nightly by default when run in this folder.
- .rustfmt.toml configures rustfmt to format code how I like it (Most importantly, format doc tests).

## Details of the main program

TODO

## Details of the solver

TODO
