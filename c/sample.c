#include <stdio.h>

#include "mtpng.h"

#define TRY(ret) { \
    mtpng_result _ret = (ret); \
    if (_ret != MTPNG_RESULT_OK) { \
        fprintf(stderr, "Error: %d\n", (int)(_ret)); \
        retval = 1; \
        goto cleanup; \
    }\
}

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


int main(int argc, char **argv) {
    printf("hello\n");

    int retval = 0;
    FILE *out = fopen("out/csample.png", "wb");

    size_t const threads = MTPNG_THREADS_DEFAULT;

    uint32_t const width = 1024;
    uint32_t const height = 768;
    mtpng_color const color_type = MTPNG_COLOR_TRUECOLOR;
    uint8_t const depth = 8;

    size_t const channels = 3;
    size_t const bpp = channels * depth / 8;
    size_t const stride = width * bpp;

    uint8_t* const data = (uint8_t*)malloc(stride * height);
    for (size_t y = 0; y < height; y++) {
        for (size_t x = 0; x < width; x++) {
            size_t i = stride * y + x * bpp;
            data[i] = (x + y) % 256;
            data[i + 1] = (2 * x + y) % 256;
            data[i + 2] = (x + 2 * y) % 256;
        }
    }

    //
    // Create a custom thread pool and the encoder.
    //
    mtpng_threadpool* pool;
    TRY(mtpng_threadpool_new(&pool, threads));

    mtpng_encoder* encoder;
    TRY(mtpng_encoder_new(&encoder,
                          write_func, flush_func, (void *)out,
                          pool));

    //
    // Set up the PNG image state
    //
    TRY(mtpng_encoder_set_size(encoder, 1024, 768));
    TRY(mtpng_encoder_set_color(encoder, color_type, depth));

    //
    // Write the data!
    //
    TRY(mtpng_encoder_write_header(encoder));
    for (size_t y = 0; y < height; y++) {
        TRY(mtpng_encoder_append_row(encoder, &data[y * stride], stride));
    }
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
