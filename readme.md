# mtpng

A parallelized PNG encoder in Rust

by Brion Vibber <brion@pobox.com>

# Background

Compressing PNG files is a relatively slow operation at large image sizes, and can take from half a second to over a second for 4K resolution and beyond. See [my blog post series on the subject](https://brionv.com/log/2018/08/29/parallelizing-png-compression-part-1/) for more details.

The biggest CPU costs in traditional libpng seem to be the filtering, which is easy to parallelize, and the deflate compression, which can be parallelized in chunks at a slight loss of compression between block boundaries.

[pigz](https://zlib.net/pigz/) is a well-known C implementation of parallelized deflate/gzip compression, and was a strong inspiration for the chunking scheme used here.

I was also inspired by an experimental C++/OpenMP project called `png-parallel` by Pascal Beyeler, which didn't implement filtering but confirmed the basic theory.

# State

Creates correct files in all color formats (input must be pre-packed). Performs well on large files, but needs work for small files and ancillary chunks. No API stability yet!

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
* SHOULD work on Windows arm, arm64 (untested)

## Compression

Compression ratio is a tiny fraction worse than libpng with the dual-4K screenshot and the [arch photo](https://raw.githubusercontent.com/brion/mtpng/master/samples/arch-640.png) at the current default 256 KiB chunk size, getting closer the larger you increase it.

Using a smaller chunk size, or enabling streaming mode, will increase the file size slightly more in exchange for greater parallelism (small chunks) and lower latency to bytes hitting the wire (streaming).

## Performance

Note that unoptimized debug builds are about 50x slower than optimized release builds. Always run with `--release`!

As of September 8, 2018 with Rust 1.28.0, single-threaded performance on Linux x86_64 is ~15-20% faster than libpng saving the same [dual-4K screenshot sample image](https://raw.githubusercontent.com/brion/mtpng/master/samples/dual4k.png) on Linux and macOS x86_64. Using multiple threads consistently beats libpng by a lot, and scales reasonably well at least to 8 physical cores.

Times for re-encoding the dual-4K screenshot at default options:

```
MacBook Pro 13" 2015
5th-gen Core i7 3.1 GHz
2 cores + Hyper-Threading

Linux x86_64:
- libpng gcc         --  850 ms (target to beat)
- libpng clang       --  900 ms
- mtpng @  1 thread  --  721 ms -- 1.0x (victory!)
- mtpng @  2 threads --  373 ms -- 1.9x
- mtpng @  4 threads --  316 ms -- 2.3x (HT)

macOS x86_64:
- libpng clang       --  943 ms (slower than Linux/gcc)
- mtpng @  1 thread  --  806 ms -- 1.0x (nice!)
- mtpng @  2 threads --  373 ms -- 1.9x
- mtpng @  4 threads --  305 ms -- 2.1x (HT)
```

macOS and Linux x86_64 perform about the same on the same machine, but libpng on macOS is built with clang, which seems to optimize libpng's filters worse than gcc does. This means we beat libpng on macOS by a larger margin than on Linux, where it's usually built with gcc.


```
Refurbed old Dell workstation
Xeon E5520 2.26 GHz
2x 4 cores + Hyper-Threading
configured for SMP (NUMA disabled)

Linux x86_64:
- libpng gcc         -- 1695 ms (target to beat)
- mtpng @  1 thread  -- 1330 ms -- 1.0x (winning!)
- mtpng @  2 threads --  698 ms -- 1.9x
- mtpng @  4 threads --  354 ms -- 3.8x
- mtpng @  8 threads --  190 ms -- 7x
- mtpng @ 16 threads --  165 ms -- 8.0x (HT)

Windows 10 x86_64:
- mtpng @  1 thread  -- 1551 ms -- 1.0x
- mtpng @  2 threads --  781 ms -- 2.0x
- mtpng @  4 threads --  402 ms -- 3.9x
- mtpng @  8 threads --  222 ms -- 6.9x
- mtpng @ 16 threads --  184 ms -- 8.4x (HT)

Windows 10 i686:
- mtpng @  1 thread  -- 1802 ms -- 1.0x
- mtpng @  2 threads --  917 ms -- 1.9x
- mtpng @  4 threads --  469 ms -- 3.8x
- mtpng @  8 threads --  259 ms -- 6.9x
- mtpng @ 16 threads --  206 ms -- 8.7x
```

Windows seems a little slower than Linux on the same machine, not quite sure why. The Linux build runs on Windows 10's WSL compatibility layer slightly slower than native Linux but faster than native Windows.

32-bit builds are a bit slower still, but I don't have a Windows libpng comparison handy.

```
Raspberry Pi 3B+
Cortex A53 1.4 GHz
4 cores

Linux armhf (Raspian):
- libpng gcc         --  5368 ms
- mtpng @  1 thread  --  6068 ms -- 1.0x
- mtpng @  2 threads --  3126 ms -- 1.9x
- mtpng @  4 threads --  1875 ms -- 3.2x

Linux aarch64 (Fedora 28):
- libpng gcc         -- 10635 ms
- mtpng @  1 thread  --  8262 ms -- 1.0x
- mtpng @  2 threads --  4239 ms -- 1.9x
- mtpng @  4 threads --  2508 ms -- 3.3x
```

On 32-bit ARM we don't quite beat libpng single-threaded, but multi-threaded still does well. 64-bit ARM does better, perhaps because libpng is less optimized there. Note this machine throttles aggressively if it heats up, making the second run of a repeat on a long file like that noticeably slower than the first.

```
iPhone X
A11 2.39 GHz
6 cores (2 big, 4 little)

iOS aarch64:
- mtpng @ 1 thread  -- 802 ms -- 1.0x
- mtpng @ 2 threads -- 475 ms -- 1.7x
- mtpng @ 4 threads -- 371 ms -- 2.2x
- mtpng @ 6 threads -- 320 ms -- 2.5x
```

A high-end 64-bit ARM system is quite a bit faster! It scales ok to 2 cores, getting smaller but real benefits from scheduling further work on the additional little cores.

## Todos

See the [projects list on GitHub](https://github.com/brion/mtpng/projects) for active details.

# Data flow

Encoding can be broken into many parallel blocks:

![Encoder data flow diagram](https://raw.githubusercontent.com/brion/mtpng/master/docs/data-flow-write.png)

Decoding cannot; it must be run as a stream, but can pipeline.

![Decoder data flow diagram](https://raw.githubusercontent.com/brion/mtpng/master/docs/data-flow-read.png)

# Dependencies

[Rayon](https://crates.io/crates/rayon) is used for its ThreadPool implementation. You can create an encoder using either the default Rayon global pool or a custom ThreadPool instance.

[crc](https://crates.io/crates/crc) is used for calculating PNG chunk checksums.

[libz-sys](https://crates.io/crates/libz-sys) is used to wrap libz for the deflate compression. I briefly looked at pure-Rust implementations but couldn't find any supporting raw stream output, dictionary setting, and flushing to byte boundaries without closing the stream.

[itertools](https://crates.io/crates/itertools) is used to manage iteration in the filters.

[png](https://crates.io/crates/png) is used by the CLI tool to load input files to recompress for testing.

[clap](https://crates.io/crates/clap) is used by the CLI tool to handle option parsing and help display.

[time](https://crates.io/crates/time) is used by the CLI tool to time compression.

[typenum](https://crates.io/crates/typenum) is used to do compile-time constant specialization via generics.

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
