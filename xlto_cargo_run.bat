
set CC=clang-cl
set CXX=clang-cl
set AR=llvm-lib

set CFLAGS_ZLIB=/D_MD -MD /clang:-flto=thin /clang:-fuse-ld=lld-link /Gy /bigobj /Oi /fp:precise /FS  -msse -msse2 -msse3 /EHa /O2 /Ob2 /DNDEBUG -DNDEBUG
set CFLAGS=%CFLAGS_ZLIB% /D_WIN32
set CXXFLAGS=%CFLAGS%

cd opencv\build\inst\include
set OPENCV_INCLUDE_PATHS=%CD%
cd ..\x64\vc16\staticlib
set OPENCV_LINK_PATHS=%CD%
REM opencv_imgproc454 is the one I actually want
set OPENCV_LINK_LIBS=opencv_core454,libopenjp2,opencv_imgproc454,libtiff,libjpeg-turbo
cd ..\..\..\..\..\..

cd tesseract\build\inst\include
set TESSERACT_INCLUDE_PATHS=%CD%
cd ..\lib
set TESSERACT_LINK_PATHS=%CD%,%OPENCV_LINK_PATHS%
set TESSERACT_LINK_LIBS=tesseract41,zlibstatic,tiff,jpeg-turbo
cd ..\..\..\..

cd leptonica\build\inst\include
set LEPTONICA_INCLUDE_PATH=%CD%
cd ..\lib
set LEPTONICA_LINK_PATHS=%CD%
set LEPTONICA_LINK_LIBS=leptonica-1.83.0
cd ..\..\..\..

REM set RUSTFLAGS=-C linker=lld-link -C target-feature=+crt-static -C linker-plugin-lto -C link-arg=/OPT:REF -C link-arg=/OPT:ICF

cargo run -Z build-std=std --profile fullrel --target=x86_64-pc-windows-msvc

REM cargo bloat --profile fullrel --target=x86_64-pc-windows-msvc