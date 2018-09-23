//
// mtpng - a multithreaded parallel PNG encoder in Rust
// sample.c - C API example
//
// Copyright (c) 2018 Brion Vibber
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//

#include <stdio.h>

#include "mtpng.h"

static size_t write_func(void* user_data, const uint8_t* bytes, size_t len)
{
    FILE* out = (FILE*)user_data;
    return fwrite(bytes, 1, len, out);
}

static bool flush_func(void* user_data)
{
    FILE* out = (FILE*)user_data;
    if (fflush(out) == 0) {
        return true;
    } else {
        return false;
    }
}

//
// Note that ol' "do/while(0)" trick. Yay C macros!
// Helps ensure consistency in if statements and stuff.
//
#define TRY(ret) \
do { \
    mtpng_result _ret = (ret); \
    if (_ret != MTPNG_RESULT_OK) { \
        fprintf(stderr, "Error: %d\n", (int)(_ret)); \
        goto cleanup; \
    }\
} while (0)

int main(int argc, char **argv) {
    size_t const threads = MTPNG_THREADS_DEFAULT;

    uint32_t const width = 1024;
    uint32_t const height = 768;

    size_t const channels = 3;
    size_t const bpp = channels;
    size_t const stride = width * bpp;

    FILE* out = fopen("out/csample.png", "wb");
    if (!out) {
        fprintf(stderr, "Error: failed to open output file\n");
        return 1;
    }

    //
    // Create a custom thread pool
    //
    mtpng_threadpool* pool = NULL;
    TRY(mtpng_threadpool_new(&pool, threads));

    //
    // Set some encoding options
    //
    mtpng_encoder_options* options = NULL;
    TRY(mtpng_encoder_options_new(&options));
    TRY(mtpng_encoder_options_set_chunk_size(options, 200000));
    TRY(mtpng_encoder_options_set_filter(options, MTPNG_FILTER_ADAPTIVE));
    TRY(mtpng_encoder_options_set_thread_pool(options, pool));

    //
    // Create the encoder.
    //
    mtpng_encoder* encoder = NULL;
    TRY(mtpng_encoder_new(&encoder,
                          write_func,
                          flush_func,
                          (void*)out,
                          options));

    //
    // Set up the PNG image state
    //
    mtpng_header* header = NULL;
    TRY(mtpng_header_new(&header));
    TRY(mtpng_header_set_size(header, 1024, 768));
    TRY(mtpng_header_set_color(header, MTPNG_COLOR_TRUECOLOR, 8));
    TRY(mtpng_encoder_write_header(encoder, header));

    //
    // Write the data!
    //
    uint8_t* bytes = malloc(stride);
    for (size_t y = 0; y < height; y++) {
        for (size_t x = 0; x < width; x++) {
            size_t i = x * bpp;
            bytes[i] = (x + y) % 256;
            bytes[i + 1] = (2 * x + y) % 256;
            bytes[i + 2] = (x + 2 * y) % 256;
        }
        TRY(mtpng_encoder_write_image_rows(encoder, bytes, stride));
    }
    free(bytes);
    bytes = NULL;
    TRY(mtpng_header_release(&header));
    TRY(mtpng_encoder_finish(&encoder));
    TRY(mtpng_encoder_options_release(&options));
    TRY(mtpng_threadpool_release(&pool));

    printf("Done.\n");
    return 0;

    // Error handler for the TRY macros:
cleanup:
    if (header) {
        mtpng_header_release(&header);
    }
    if (encoder) {
        mtpng_encoder_release(&encoder);
    }
    if (options) {
        mtpng_encoder_options_release(&options);
    }
    if (pool) {
        mtpng_threadpool_release(&pool);
    }

    printf("Failed!\n");
    return 1;
}
