//
// mtpng - a multithreaded parallel PNG encoder in Rust
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

#ifndef MTPNG_H_INCLUDED
#define MTPNG_H_INCLUDED 1

#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>


#pragma mark Consts and enums

#define MTPNG_THREADS_DEFAULT 0

typedef enum mtpng_result_t {
    MTPNG_RESULT_OK = 0,
    MTPNG_RESULT_ERR = 1
} mtpng_result;

typedef enum mtpng_filter_t {
    MTPNG_FILTER_NONE = 0,
    MTPNG_FILTER_SUB = 1,
    MTPNG_FILTER_UP = 2,
    MTPNG_FILTER_AVERAGE = 3,
    MTPNG_FILTER_PAETH = 4
} mtpng_filter;

typedef enum mtpng_color_t {
    MTPNG_COLOR_GREYSCALE = 0,
    MTPNG_COLOR_TRUECOLOR = 2,
    MTPNG_COLOR_INDEXED_COLOR = 3,
    MTPNG_COLOR_GREYSCALE_ALPHA = 4,
    MTPNG_COLOR_TRUECOLOR_ALPHA = 6
} mtpng_color;

#pragma mark Structs

//
// Opaque structs for the threadpool and encoder.
//
typedef struct mtpng_threadpool_struct mtpng_threadpool;
typedef struct mtpng_encoder_struct mtpng_encoder;

#pragma mark Function types

typedef size_t (*mtpng_write_func)(void* user_data, const uint8_t* bytes, size_t len);

typedef bool (*mtpng_flush_func)(void* user_data);

#pragma mark ThreadPool

//
// Creates a new threadpool with the given number
// of threads. MTPNG_THREADS_DEFAULT (0) means to
// auto-detect the number of logical processors.
//
extern mtpng_result
mtpng_threadpool_new(mtpng_threadpool** pool,
                     size_t threads);

//
// Releases the pool's memory and clears the pointer.
//
extern mtpng_result
mtpng_threadpool_release(mtpng_threadpool** pool);

#pragma mark Encoder

//
// Create a new PNG encoder instance.
//
// The write and flush functions are required, and must not be NULL.
// @fixme enforce that
//
// user_data is passed to the write and flush functions, and may
// be any value such as a private object pointer or NULL.
//
// p_pool may be NULL, in which case a default global thread pool
// will be used.
//
extern mtpng_result
mtpng_encoder_new(mtpng_encoder** pp_encoder,
                  mtpng_write_func write_func,
                  mtpng_flush_func flush_func,
                  void* const  user_data,
                  mtpng_threadpool *p_pool);

//
// Releases the encoder's memory and clears the pointer.
//
// If using a threadpool, must be called before releasing the
// threadpool!
//
// If the encoder is still in use, this may explode.
//
extern mtpng_result
mtpng_encoder_release(mtpng_encoder** pp_encoder);

//
// Set the color type and depth for the image.
//
// If you do not specify, you'll get truecolor with alpha
// at 8-bit depth.
//
// Must be called before mtpng_encoder_write_header().
//
extern mtpng_result
mtpng_encoder_set_size(mtpng_encoder* p_encoder,
                       uint32_t width,
                       uint32_t height);

//
// Set the color type and depth for the image.
//
// If you do not specify, you'll get truecolor with alpha
// at 8-bit depth.
//
// Must be called before mtpng_encoder_write_header().
//
extern mtpng_result
mtpng_encoder_set_color(mtpng_encoder* p_encoder,
                        mtpng_color color_type,
                        uint8_t depth);

//
// Override the default chunk size for parallel encoding
// of larger files. Actual chunking will be in terms of
// rows, so data chunks will be at least the given size
// in bytes.
//
// If there are more chunks in the image's raw data bytes
// than available CPUs on the thread pool, you should see
// parallel speedups as long as input data is provided
// fast enough.
//
// If the file is smaller than the chunk size, currently
// the speed will be equivalent to running single-threaded.
//
// chunk_size must be at least 32768 bytes, required for
// maintaining compression across chunks.
//
// Must be called before mtpng_encoder_write_header().
//
extern mtpng_result
mtpng_encoder_set_chunk_size(mtpng_encoder* p_encoder,
                             size_t chunk_size);

//
// Signal that we're done setting up, and start writing
// header data to the output.
//
// Must be called before mtpng_encoder_append_row() or
// mtpng_encoder_finish().
//
extern mtpng_result
mtpng_encoder_write_header(mtpng_encoder *p_encoder);

//
// Append a row of data. Must be called after the call to
// mtpng_encoder_write_header() and before the call to
// mtpng_encoder_finish().
//
// Must be called after mtpng_encoder_write_header and before
// mtpng_encoder_apend_row() or mtpng_encoder_finish(). Call
// once for each row of the image data.
//
// Image data must be pre-packed in the correct bit depth and
// chanel order.
//
extern mtpng_result
mtpng_encoder_append_row(mtpng_encoder* p_encoder,
                         uint8_t* p_bytes,
                         size_t len);

//
// Wait for any outstanding work blocks, flush output,
// release the encoder instance and clear the pointer.
//
// Must be called after all rows have been appended with
// mtpng_encoder_append_row().
//
// You do not need to call mtpng_encoder_release after
// this returns, and should not try.
//
// If using a threadpool, must be called before releasing
// the threadpool!
//
extern mtpng_result
mtpng_encoder_finish(mtpng_encoder** pp_encoder);

#pragma mark footer


#endif /* MTPNG_H_INCLUDED */
