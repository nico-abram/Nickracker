// This file contains a wrapper in C++ that exposes a function with a C ABI to call a C++ method on a TessBaseAPI handle
// See ocr.rs for usage and build.rs for the build script

#include <tesseract/baseapi.h>

extern "C"
{
    int TessBaseAPI_CustomInitStub(void *handle, const char *data, int data_size, const char *language,
                                   unsigned int mode, char **configs, int configs_size, bool set_only_non_debug_params)
    {
        tesseract::TessBaseAPI *thandle = (tesseract::TessBaseAPI *)handle;
        tesseract::OcrEngineMode tmode = (tesseract::OcrEngineMode)mode;
        return thandle->Init(data, data_size, language, tmode, configs, configs_size, nullptr, nullptr, set_only_non_debug_params, nullptr);
    }
}