# mtpng

A parallelized PNG encoder in Rust

by Brion Vibber <brion@pobox.com>

# Background

Compressing PNG files is a relatively slow operation at large image sizes, and can take from half a second to over a second for 4K resolution and beyond.

The biggest CPU costs in traditional libpng seem to be the filtering, which is easy to parallelize, and the deflate compression, which can be parallelized in chunks at a slight loss of compression between block boundaries.

[pigz](https://zlib.net/pigz/) is a well-known C implementation of parallelized deflate/gzip compression, and was a strong inspiration for the chunking scheme used here.

I was also inspired by an experimental C++/OpenMP project called `png-parallel` by Pascal Beyeler, which didn't implement filtering but confirmed the basic theory.

# State

Currently very unfinished, but more or less works. Not yet optimized or made usable.

# Performance

Note that unoptimized debug builds are about 25x slower than optimized release builds. Always run with `--release`!

As of September 5, 2018 with Rust 1.28.0, single-threaded performance is ~20% faster than gdk-pixbuf + libpng on the test images in the samples folder, but ~20% slower than php + gd + libpng saving the same images.

Compression is a bit better than libpng with the dual-4K screenshot, and a bit worse on the arch photo.

Scaling is pretty good up to 8 physical cores.

Times for re-encoding the dual-4K screenshot at default options:

```
MacBook Pro 13" 2015
5th-gen Core i7 3.1 GHz
2 cores + Hyper-Threading

libgd + libpng     --  974 ms (target!)

mtpng @  1 thread  -- 1011 ms -- 1.0x
mtpng @  2 threads --  526 ms -- 1.9x
mtpng @  4 threads --  453 ms -- 2.2x (HT)
```

```
Refurbed old Dell workstation
Xeon E5520 2.26 GHz
2x 4 cores + Hyper-Threading

libgd + libpng     -- 1695 ms (target!)
gdkpixbuf + libpng -- 2308 ms (why slower?)

mtpng @  1 thread  -- 2068 ms -- 1.0x
mtpng @  2 threads -- 1028 ms -- 2.0x
mtpng @  4 threads --  528 ms -- 3.9x
mtpng @  8 threads --  302 ms -- 6.8x
mtpng @ 16 threads --  272 ms -- 7.6x (HT)
```

# Todos

Immediate todos:
* benchmark and optimize
* compare compression tradeoffs for different chunk sizes

Soon todos:
* allow buffering into a single IDAT chunk if not streaming

In a bit todos:
* clean up error handling and builder patterns
* start figuring out a public-facing api
* publish crate
* allow blocking on full thread pool to share resources more nicely

Someday todos:
* helpers for packing pixels from non-native formats
* interlacing support
* reading support with two-thread pipeline (see data flow diagram below)

# Data flow

Encoding can be broken into many parallel blocks:

![Encoder data flow diagram](https://raw.githubusercontent.com/brion/mtpng/master/docs/data-flow-write.png)

Decoding cannot; it must be run as a stream, but can pipeline.

![Decoder data flow diagram](https://raw.githubusercontent.com/brion/mtpng/master/docs/data-flow-read.png)

# Dependencies

[Rayon](https://crates.io/crates/rayon) is used for its ThreadPool implementation. You can create an encoder using either the default Rayon global pool or a custom ThreadPool instance.

[crc](https://crates.io/crates/crc) is used for calculating PNG chunk checksums.

[libz-sys](https://crates.io/crates/libz-sys) is used to wrap libz for the deflate compression. I briefly looked at pure-Rust implementations but couldn't find any supporting raw stream output, dictionary setting, and flushing to byte boundaries without closing the stream.

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