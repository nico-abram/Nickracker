CALL "C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\VC\Auxiliary\Build\vcvars64.bat"

set CC=clang-cl
set CXX=clang-cl
set AR=llvm-lib

set CFLAGS_ZLIB=/D_MD -MD /clang:-flto=thin /clang:-fuse-ld=lld-link /Gy /bigobj /Oi /fp:precise /FS  -msse -msse2 -msse3 /EHa /O2 /Ob2 /DNDEBUG  -DNDEBUG
set CFLAGS=%CFLAGS_ZLIB% /D_WIN32
set CXXFLAGS=%CFLAGS%

set _COMMON_CMAKE=-DCMAKE_LINKER=lld-link -DCMAKE_CXX_COMPILER=%CC% -DCMAKE_C_COMPILER=%CXX% -DCMAKE_BUILD_TYPE=Release -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL
set COMMON_CMAKE=%_COMMON_CMAKE% "-DCMAKE_CXX_FLAGS=%CXXFLAGS%" "-DCMAKE_C_FLAGS=%CFLAGS%" "-DCMAKE_INSTALL_PREFIX=./inst"
set ZLIB_CMAKE=%_COMMON_CMAKE% "-DCMAKE_CXX_FLAGS=%CFLAGS_ZLIB%" "-DCMAKE_C_FLAGS=%CFLAGS_ZLIB%" 

mkdir tesseract\build\inst\include
pushd tesseract\build\inst\include
set ZLIB_INC=%CD%
set "ZLIB_INC=%ZLIB_INC:\=/%"
popd
mkdir tesseract\build\inst\lib
pushd tesseract\build\inst\lib
set ZLIB_LIB=%CD%\zlibstatic.lib
set "ZLIB_LIB=%ZLIB_LIB:\=/%"
popd

If "%1"=="zlib" (goto zlib)
If "%2"=="zlib" (goto zlib)
If "%1"=="opencv" (goto opencv)
If "%2"=="opencv" (goto opencv)
If "%1"=="leptonica" (goto leptonica)
If "%2"=="leptonica" (goto leptonica)

REM Make the tesseract build folder early so we can put the zlib lib in there
cd tesseract
rmdir /s /q build
mkdir build/inst
cd ..

If "%1"=="tesseract" (goto tesseract)
If "%2"=="tesseract" (goto tesseract)

:zlib

cd zlib
rmdir /s /q build
mkdir build
cd build
cmake %ZLIB_CMAKE% "-DCMAKE_INSTALL_PREFIX=./../../tesseract/build/inst" -G "Ninja" ..
ninja
cmake -P cmake_install.cmake
cd ../..

If "%1"=="only" (exit)
:opencv

cd opencv
rmdir /s /q build
mkdir build
cd build

REM We don't use opencv_imgcodecs but we want to build jpeg and tiff for leptonica
cmake %COMMON_CMAKE% -DWITH_ITT=OFF -DBUILD_opencv_ml=OFF -DBUILD_opencv_flann=OFF -DBUILD_opencv_dnn=OFF -DBUILD_opencv_gapi=OFF -DBUILD_opencv_photo=OFF -DBUILD_OPENEXR=OFF -DWITH_IMGCODEC_HDR=OFF -DWITH_IMGCODEC_PFM=OFF -DWITH_IMGCODEC_PXM=OFF -DWITH_IMGCODEC_SUNRASTER=OFF -DBUILD_opencv_imgcodecs=ON -DWITH_WIN32UI=OFF -DBUILD_opencv_calib3d=OFF -DBUILD_opencv_highgui=OFF -DBUILD_opencv_video=OFF -DBUILD_opencv_videoio=OFF -DWITH_IPP=OFF -DBUILD_WITH_STATIC_CRT=OFF -DBUILD_IPP_IW=OFF -DBUILD_PROTOBUF=ON -DWITH_PROTOBUF=ON -DBUILD_opencv_apps=OFF -DBUILD_ZLIB=OFF "-DZLIB_INCLUDE_DIR=%ZLIB_INC%" "-DZLIB_LIBRARY=%ZLIB_LIB%" -DBUILD_WEBP=OFF -DWITH_WEBP=OFF -DBUILD_PACKAGE=OFF -DBUILD_PERF_TESTS=OFF -DBUILD_JAVA=OFF -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTS=OFF -DBUILD_opencv_js=OFF -DBUILD_opencv_java_bindings_generator=OFF -DBUILD_opencv_js_bindings_generator=OFF -DBUILD_opencv_objc_bindings_generator=OFF -DBUILD_opencv_python_bindings_generator=OFF -DBUILD_opencv_python_tests=OFF -DBUILD_opencv_ts=OFF -DWITH_DIRECTX=OFF -DWITH_DSHOW=OFF -DWITH_FFMPEG=OFF -DWITH_GSTREAMER=OFF -DWITH_EXR=OFF -G "Ninja" ..

set CL=/clang:-Wno-everything
ninja
set CL=
cmake -P cmake_install.cmake
cd ../..

cd opencv\build\inst\include
set OPENCV_INCLUDE_PATHS=%CD%

cd ..\x64\vc16\staticlib
set OPENCV_LINK_PATHS=%CD%

REM opencv-rust is annoying and removes the lib prefix, so we remove it from the lib files
ren libpng.lib png.lib
ren libtiff.lib tiff.lib
ren libprotobuf.lib protobuf.lib
ren libopenjp2.lib openjp2.lib
ren libjpeg-turbo.lib jpeg-turbo.lib
set OPENCV_LINK_LIBS=opencv_core454,libopenjp2,opencv_imgproc454,libtiff,libjpeg-turbo
REM ,ade,ittnotify,ippiw,ippicvmt,opencv_flann454,opencv_photo454,opencv_ml454,opencv_features2d454,opencv_gapi454,opencv_highgui454,opencv_objdetect454,opencv_imgcodecs454,opencv_calib3d454,opencv_dnn454,opencv_stitching454,opencv_video454,opencv_videoio454,IlmImf,libjpeg-turbo,quirc,libpng,libprotobuf,libtiff
cd ..\..\..\..\..\..

If "%1"=="only" (exit)
:leptonica

cd leptonica
rmdir /s /q build
mkdir build
cd build
pushd ..\..\opencv\build\inst\x64\vc16\staticlib
set TIFF_LIB=%CD%
popd
pushd ..\..\opencv\build\3rdparty\libtiff
set TIFF_INC=%CD%
popd
pushd ..\..\opencv\3rdparty\libtiff
set TIFF_INC=%TIFF_INC%;%CD%
popd
cmake %COMMON_CMAKE% "-DTIFF_INCLUDE_DIR=%TIFF_INC%" "-DTIFF_LIBRARY=%TIFF_LIB%\tiff.lib;%TIFF_LIB%\jpeg-turbo.lib" "-DZLIB_INCLUDE_DIR=%ZLIB_INC%" "-DZLIB_LIBRARY=%ZLIB_LIB%" -DSW_BUILD=OFF -DBUILD_PROG=OFF -G "Ninja" ..
set CL=/clang:-Wno-everything
ninja
set CL=
cmake -P cmake_install.cmake
cd ../..

If "%1"=="only" (exit)
:tesseract

cd tesseract
cd build
pushd .\..\..\leptonica\build
set CMAKE_PREFIX_PATH=%CD%
popd
cmake %COMMON_CMAKE% -DCMAKE_PREFIX_PATH=%CMAKE_PREFIX_PATH% -DLeptonica_DIR=%CMAKE_PREFIX_PATH% -DCPPAN_BUILD=OFF -DBUILD_TRAINING_TOOLS=OFF -DSTATIC=ON -G "Ninja" ..
set CL=/clang:-Wno-everything
ninja
set CL=
cmake -P cmake_install.cmake
cd ../..

If "%1"=="only" (exit)


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

cargo clean --profile fullrel --target=x86_64-pc-windows-msvc
cargo build --profile fullrel --target=x86_64-pc-windows-msvc

