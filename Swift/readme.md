# MTPNGCodec

MTPNGCodec framework provides a Swift API wrapper on top of the C API wrapper for the Rust mtpng multithreaded PNG codec.

Currently this provides a bare-bones MTPNGEncoder which can produce an output stream that will have a compression level similar to libpng, but usually at much lower latency.

The Swift API uses `Span<Uint8>`s to wrap temporary references to source and output data safely.

## Installation

Run `build-rust-libs.sh` to build the Rust libraries into `Swift/lib/*` first!

Work in progress at making into a conveniently usable package dependency.

## Usage

To control the thread count, you can optionally create a thread pool to connect encoders and decoders to. If you don't specify a thread pool, a default will be created using all available logical processors.

```swift
let pool = try MTPNGThreadPool(threads: n)
let options = MTPNGEncoderOptions()
try options.setThreadPool(pool: pool)
```

Then you create your encoder, which currently is this horrible monstrosity:

```swift
let encoder = try MTPNGEncoder(
    write: { (bytes: Span<UInt8>) -> Int in
        bytes.withUnsafeBufferPointer { buffer in
            outputBuffer.append(contentsOf: buffer)
        }
        return bytes.count;
    },
    flush: nil,
    options: options)
```

This is all terrible and I should probably be using a more idiomatic output stream interface for the write and flush functions (which are shown here just appending to a buffer).

An encoder object will live for a single encode only, but encoder options can be reused across multiple encoders.

Now you'll set up the PNG header with some global info on the image and write it out. Again you can reuse MTPNGHeaders across multiple encodes of same-spec images:

```swift
let header = try MTPNGHeader()
try header.setSize(width: UInt32(width), height: UInt32(height))
try header.setColor(color: MTPNGColor.TruecolorAlpha, bits: 8)
try encoder.writeHeader(header: header)
```

And now put your raw image data in! If it's already in the correct format you can just dump it in in one big `Span<Uint8>`:

```swift
try encoder.writeImageRows(bytes: data)
try encoder.finish()
```

The queue is "greedy" and will accumulate as much data as you are willing to put in, as this will maximize available work material for parallel jobs, however this means you should plan for peak memory usage of at least double the size of your raw image plus the compressed size (for everything chunked and queued up, filtered, and producing output).

## Error handling

Virtually every operation could fail given invalid data. If we trust the internal APIs to be operating correctly we could reduce some of these throws cases and remove the annoying "try"s.

Currently MTPNGError only ever returns a single failure case because the C API doesn't expose anything more than a binary success/fail yet! This needs to be fixed on the C API end and extended through to Swift.

## Memory safety

The wrapper class types (MTPNGThreadPool, MTPNGEncoderOptions, MTPNGHeader, and MTPNGEncoder) will retain references to each other and should not be able to explode. References into the Rust output buffers are passed to the write function as `Span<Uint8>` and should not be able to outlive the function unless you do unsafe stuff with them.

The underlying Rust code is... probably safe? :D The C API to the Rust code is ... hopefully right but lets you shoot yourself in the foot! :D The Swift wrappers on the C API are .... hopefully right? :D

