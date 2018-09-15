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

typedef struct main_state_t {
    FILE* out;
    size_t width;
    size_t bpp;
    size_t stride;
    size_t y;
} main_state;

static size_t read_func(void* user_data, uint8_t* bytes, size_t len)
{
    main_state* state = (main_state*)user_data;
    for (size_t x = 0; x < state->width; x++) {
        size_t i = state->stride * state->y + x * state->bpp;
        bytes[i] = (x + state->y) % 256;
        bytes[i + 1] = (2 * x + state->y) % 256;
        bytes[i + 2] = (x + 2 * state->y) % 256;
    }
    state->y++;
    return len;
}

static size_t write_func(void* user_data, const uint8_t* bytes, size_t len)
{
    main_state* state = (main_state*)user_data;
    return fwrite(bytes, 1, len, state->out);
}

static bool flush_func(void* user_data)
{
    main_state* state = (main_state*)user_data;
    if (fflush(state->out) == 0) {
        return true;
    } else {
        return false;
    }
}

#define TRY(ret) { \
    mtpng_result _ret = (ret); \
    if (_ret != MTPNG_RESULT_OK) { \
        fprintf(stderr, "Error: %d\n", (int)(_ret)); \
        retval = 1; \
        goto cleanup; \
    }\
}

int main(int argc, char **argv) {
    printf("hello\n");

    int retval = 0;
    main_state state;
    state.out = fopen("out/csample.png", "wb");

    size_t const threads = MTPNG_THREADS_DEFAULT;

    uint32_t const width = 1024;
    uint32_t const height = 768;
    mtpng_color const color_type = MTPNG_COLOR_TRUECOLOR;
    uint8_t const depth = 8;

    size_t const channels = 3;
    size_t const bpp = channels * depth / 8;
    size_t const stride = width * bpp;

    state.stride = stride;
    state.width = width;
    state.bpp = bpp;
    state.y = 0;

    //
    // Create a custom thread pool and the encoder.
    //
    mtpng_threadpool* pool;
    TRY(mtpng_threadpool_new(&pool, threads));

    mtpng_encoder* encoder;
    TRY(mtpng_encoder_new(&encoder,
                          read_func,
                          write_func,
                          flush_func,
                          (void *)&state,
                          pool));

    //
    // Set some encoding options
    //
    TRY(mtpng_encoder_set_chunk_size(encoder, 200000));

    //
    // Set up the PNG image state
    //
    TRY(mtpng_encoder_set_size(encoder, 1024, 768));
    TRY(mtpng_encoder_set_color(encoder, color_type, depth));

    //
    // Write the data!
    //
    TRY(mtpng_encoder_write_header(encoder));
    TRY(mtpng_encoder_write_image_data());
    TRY(mtpng_encoder_finish(&encoder));

cleanup:
    if (encoder) {
        TRY(mtpng_encoder_release(&encoder));
    }
    if (pool) {
        TRY(mtpng_threadpool_release(&pool));
    }

    printf("goodbye\n");
    return retval;
}
