# mtpng

A parallelized PNG encoder in Rust

by Brion Vibber <brion@pobox.com>

# Background

Compressing PNG files is a relatively slow operation at large image sizes, and can take from half a second to over a second for 4K resolution and beyond.

The biggest CPU costs in traditional libpng seem to be the filtering, which is easy to parallelize, and the deflate compression, which can be parallelized in chunks at a slight loss of compression between block boundaries.

[pigz](https://zlib.net/pigz/) is a well-known C implementation of parallelized deflate/gzip compression, and was a strong inspiration for the chunking scheme used here.

I was also inspired by an experimental C++/OpenMP project called `png-parallel` by Pascal Beyeler, which didn't implement filtering but confirmed the basic theory.

# State

Currently very unfinished.

The code mocks up the basic flow of data blocks through a Rayon ThreadPool, but doesn't actually write or compress anything correctly.

# Data flow

![Data flow diagram](https://raw.githubusercontent.com/brion/mtpng/master/png-data-flow.png)

# Dependencies

[Rayon](https://crates.io/crates/rayon) is used for its ThreadPool implementation. You can create an encoder using either the default Rayon global pool or a custom ThreadPool instance.

[crc](https://crates.io/crates/crc) is used for calculating PNG chunk checksums.

# Copyright

Don't touch it until I put that license on it! ;)
