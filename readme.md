# mtpng

A parallelized PNG encoder in Rust

by Brion Vibber <brion@pobox.com>

# Background

Compressing PNG files is a relatively slow operation at large image sizes, and can take from half a second to over a second for 4K resolution and beyond. See [my blog post series on the subject](https://brionv.com/log/2018/08/29/parallelizing-png-compression-part-1/) for more details.

The biggest CPU costs in traditional libpng seem to be the filtering, which is easy to parallelize, and the deflate compression, which can be parallelized in chunks at a slight loss of compression between block boundaries.

[pigz](https://zlib.net/pigz/) is a well-known C implementation of parallelized deflate/gzip compression, and was a strong inspiration for the chunking scheme used here.

I was also inspired by an experimental C++/OpenMP project called `png-parallel` by Pascal Beyeler, which didn't implement filtering but confirmed the basic theory.

# State

Creates correct files in all color formats (input must be pre-packed). Performs well on large files, but needs work for small files and ancillary chunks. Planning API stability soon, but not yet there -- things will change before 1.0.

## Goals

Performance:
* ☑️ MUST be faster than libpng when multi-threaded
* ☑️ SHOULD be as fast as or faster than libpng when single-threaded

Functionality:
* ☑️ MUST support all standard color types and depths
* ☑️ MUST support all standard filter modes
* ☑️ MUST compress within a few percent as well as libpng
* MAY achieve better compression than libpng, but MUST NOT do so at the cost of performance
* ☑️ SHOULD support streaming output
* MAY support interlacing

Compatibility:
* MUST have a good Rust API (in progress)
* MUST have a good C API (in progress)
* ☑️ MUST work on Linux x86, x86_64
* ☑️ MUST work on Linux arm, arm64
* ☑️ SHOULD work on macOS x86_64
* ☑️ SHOULD work on iOS arm64
* ☑️ SHOULD work on Windows x86, x86_64
* ☑️️ SHOULD work on Windows arm64

## Compression

Compression ratio is a tiny fraction worse than libpng with the dual-4K screenshot and the [arch photo](https://raw.githubusercontent.com/brion/mtpng/master/samples/arch-640.png) at the current default 256 KiB chunk size, getting closer the larger you increase it.

Using a smaller chunk size, or enabling streaming mode, will increase the file size slightly more in exchange for greater parallelism (small chunks) and lower latency to bytes hitting the wire (streaming).

## Performance

Note that unoptimized debug builds are about 50x slower than optimized release builds. Always run with `--release`!

As of September 26, 2018 with Rust 1.29.0, single-threaded performance on Linux x86_64 is ~30-40% faster than libpng saving the same [dual-4K screenshot sample image](https://raw.githubusercontent.com/brion/mtpng/master/samples/dual4k.png) on Linux and macOS x86_64. Using multiple threads consistently beats libpng by a lot, and scales reasonably well at least to 8 physical cores.

See [docs/perf.md](https://github.com/brion/mtpng/blob/master/docs/perf.md) for informal benchmarks on various devices.

At the default settings, files whose uncompressed data is less than 128 KiB will not see any multi-threading gains, but may still run faster than libpng due to faster filtering.

## Todos

See the [projects list on GitHub](https://github.com/brion/mtpng/projects) for active details.

# Usage

Note: the Rust and C APIs are not yet stable, and will change before 1.0.

## Rust usage

See the [crate API docs](https://docs.rs/mtpng/latest/mtpng/) for details.

The [mtpng CLI tool](https://github.com/brion/mtpng/blob/master/src/bin/mtpng.rs) can be used as an example of writing files.

In short, something like this:

```rust
let mut writer = Vec::<u8>::new();

let mut header = Header::new();
header.set_size(640, 480)?;
header.set_color(ColorType::TruecolorAlpha, 8)?;

let mut options = Options::new();

let mut encoder = Encoder::new(writer, &options);

encoder.write_header(&header)?;
encoder.write_image_rows(&data)?;
encoder.finish()?;
```

## C usage

See [c/mtpng.h](https://github.com/brion/mtpng/blob/master/c/mtpng.h) for a C header file which connects to unsafe-Rust wrapper functions in the [mtpng::capi](https://github.com/brion/mtpng/blob/master/src/capi.rs) module.

To build the C sample on Linux or macOS, run `make`. On Windows, run `build-win.bat x64` for an x86-64 native build, or pass `x86` or `arm64` to build for those platforms.

These will build a `sample` executable from [sample.c](https://github.com/brion/mtpng/blob/master/c/sample.c) as well as a `libmtpng.so`, `libmtpng.dylib`, or `mtpng.dll` for it to link. It produces an output file in `out/csample.png`.

# Data flow

Encoding can be broken into many parallel blocks:

![Encoder data flow diagram](https://raw.githubusercontent.com/brion/mtpng/master/docs/data-flow-write.png)

Decoding cannot; it must be run as a stream, but can pipeline (not yet implemented):

![Decoder data flow diagram](https://raw.githubusercontent.com/brion/mtpng/master/docs/data-flow-read.png)

# Dependencies

[Rayon](https://crates.io/crates/rayon) is used for its ThreadPool implementation. You can create an encoder using either the default Rayon global pool or a custom ThreadPool instance.

[crc](https://crates.io/crates/crc) is used for calculating PNG chunk checksums.

[libz-sys](https://crates.io/crates/libz-sys) is used to wrap libz for the deflate compression. I briefly looked at pure-Rust implementations but couldn't find any supporting raw stream output, dictionary setting, and flushing to byte boundaries without closing the stream.

[itertools](https://crates.io/crates/itertools) is used to manage iteration in the filters.

[typenum](https://crates.io/crates/typenum) is used to do compile-time constant specialization via generics.

[png](https://crates.io/crates/png) is used by the CLI tool to load input files to recompress for testing.

[clap](https://crates.io/crates/clap) is used by the CLI tool to handle option parsing and help display.

[time](https://crates.io/crates/time) is used by the CLI tool to time compression.

# License

You may use this software under the following MIT-style license:

Copyright (c) 2018 Brion Vibber

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
