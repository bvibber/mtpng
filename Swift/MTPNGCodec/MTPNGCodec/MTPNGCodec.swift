//
//  MTPNGCodec.swift
//  MTPNGCodec
//
//  Created by Brooke on 6/15/26.
//  Copyright © 2026 Brooke Vibber. All rights reserved.
//

import Foundation
import mtpng

public enum MTPNGError: Error {
    case unknownError
    case inputOutOfBounds
}

//
// Filter types for MTPNGEncoderOptions.setFilterMode().
//
// MTPNG_FILTER_ADAPTIVE is the default behavior, which uses
// a heuristic to try to guess the best compressing filter.
//
public enum MTPNGFilter: Int32 {
    case Adaptive = -1
    case None = 0
    case Sub = 1
    case Up = 2
    case Average = 3
    case Paeth = 4
}

//
// Strategy types for MTPNGEncoderOptions.setStrategy().
//
// Adaptive is the default behavior.
//
public enum MTPNGStrategy: Int32 {
    case Adaptive = -1
    case Default = 0
    case Filtered = 1
    case Huffman = 2
    case RLE = 3
    case Fixed = 4
}

//
// Compression levels for MTPNGEncoderOptions.setCompressionLevel().
//
public enum MTPNGCompressionLevel: Int32 {
    case Fast = 1
    case Default = 6
    case High = 9
}

//
// Color types for MTPNGEncoderOptions.setColor().
//

public enum MTPNGColor: UInt32 {
    case Greyscale = 0
    case Truecolor = 2
    case IndexedColor = 3
    case GreyscaleAlpha = 4
    case TruecolorAlpha = 6
}

public typealias MTPNGWriteFunc = (_: Span<UInt8>) -> Int

public typealias MTPNGFlushFunc = () -> Bool

public class MTPNGThreadPool {
    var handle: OpaquePointer? = nil

    public init(threads: Int) throws {
        let ret = mtpng_threadpool_new(&self.handle, threads)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
        if self.handle == nil {
            throw MTPNGError.unknownError
        }
    }
    
    deinit {
        let ret = mtpng_threadpool_release(&handle)
        if ret != MTPNG_RESULT_OK {
            print("Unexpected failure in mtpng_threadpool_release")
        }
        if self.handle != nil {
            print("Invalid state after mtpng_threadpool_release")
        }
    }
}

public class MTPNGEncoderOptions {
    var handle: OpaquePointer? = nil
    var pool: MTPNGThreadPool? = nil

    // Can throw MTPNGError.unknownError
    //
    public init() throws {
        let ret = mtpng_encoder_options_new(&self.handle)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
        if self.handle == nil {
            throw MTPNGError.unknownError
        }
    }

    deinit {
        let ret = mtpng_encoder_options_release(&handle)
        if ret != MTPNG_RESULT_OK {
            print("Unexpected failure in mtpng_encoder_options_release")
        }
        if self.handle != nil {
            print("Invalid state after mtpng_encoder_options_release")
        }
    }
    
    //
    // Set the thread pool instance to queue work on.
    //
    // pool may be nil, in which case a default global thread pool
    // will be used.
    //
    // Can throw MTPNGError.unknownError
    //
    public func setThreadPool(pool: MTPNGThreadPool?) throws {
        self.pool = pool
        let ret = mtpng_encoder_options_set_thread_pool(handle, pool?.handle)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }

    //
    // Override the default PNG filter mode selection.
    //
    // The default is .None for indexed images and
    // .Adaptive for all others. Some images compress
    // better with a particular filter.
    //
    // Can throw MTPNGError.unknownError
    //
    public func setFilter(filter: MTPNGFilter) throws {
        let ret = mtpng_encoder_options_set_filter(handle, mtpng_filter(filter.rawValue))
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }

    //
    // Override the default PNG strategy mode selection.
    //
    // Can throw MTPNGError.unknownError
    //
    public func setStrategy(strategy: MTPNGStrategy) throws {
        let ret = mtpng_encoder_options_set_strategy(handle, mtpng_strategy(strategy.rawValue))
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }

    //
    // Override the default PNG compression level.
    //
    // Default, Low, and High are available in MTPNGCompressionLevel
    // or use integers 0-9.
    //
    // Can throw MTPNGError.unknownError
    //
    public func setCompressionLevel(level: Int) throws {
        let ret = mtpng_encoder_options_set_compression_level(handle, Int32(level))
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }

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
    // Can throw MTPNGError.unknownError
    //
    public func setChunkSize(size: Int) throws {
        let ret = mtpng_encoder_options_set_chunk_size(handle, size)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }
}

public class MTPNGHeader {
    var handle: OpaquePointer? = nil
    
    // Can throw MTPNGError.unknownError
    //
    public init () throws {
        let ret = mtpng_header_new(&self.handle)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
        if self.handle == nil {
            throw MTPNGError.unknownError
        }
    }

    deinit {
        let ret = mtpng_header_release(&self.handle)
        if ret != MTPNG_RESULT_OK {
            print("Unexpected failure in mtpng_header_release!")
        }
        if self.handle != nil {
            print("Invalid state after mtpng_header_release!")
        }
    }

    // Can throw MTPNGError.unknownError
    //
    public func setSize(width: UInt32, height: UInt32) throws {
        let ret = mtpng_header_set_size(self.handle, width, height)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.inputOutOfBounds
        }
    }

    // Can throw MTPNGError.unknownError
    //
    public func setColor(color: MTPNGColor, bits: UInt8) throws {
        let ret = mtpng_header_set_color(self.handle, mtpng_color_t(UInt32(color.rawValue)), bits)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }
}

private func write_func(_user_data: UnsafeMutableRawPointer?, noisolation _bytes: UnsafePointer<UInt8>?, noisolation _len: Int) -> Int
{
    let myself = Unmanaged<MTPNGEncoder>.fromOpaque(_user_data!).takeUnretainedValue()
    if let writeFunc = myself.writeFunc {
        nonisolated(unsafe) let bytes = UnsafeBufferPointer(start: _bytes, count: _len)
        return writeFunc(bytes.span)
    } else {
        return _len
    }
}

private func flush_func(_user_data: UnsafeMutableRawPointer?) -> Bool
{
    let myself = Unmanaged<MTPNGEncoder>.fromOpaque(_user_data!).takeUnretainedValue()
    if let flushFunc = myself.flushFunc {
        return flushFunc()
    } else {
        return true
    }
}

public class MTPNGEncoder {
    var handle: OpaquePointer? = nil
    var writeFunc: MTPNGWriteFunc? = nil
    var flushFunc: MTPNGFlushFunc? = nil

    // Keep a thread pool reference for lifetime management
    var pool: MTPNGThreadPool? = nil;

    // Can throw MTPNGError.unknownError
    //
    public init (write: MTPNGWriteFunc?, flush: MTPNGFlushFunc?, options: MTPNGEncoderOptions?) throws {
        self.writeFunc = write
        self.flushFunc = flush
        self.pool = options?.pool
        let user_data = UnsafeMutableRawPointer.init(Unmanaged.passUnretained(self).toOpaque());
        let ret = mtpng_encoder_new(&handle,
                          write_func,
                          flush_func,
                          user_data,
                          options?.handle)
        if ret != MTPNG_RESULT_OK {
            print("error in mtpng_encoder_new")
            throw MTPNGError.unknownError
        }
        if handle == nil {
            print("failed to allocate in mtpng_encoder_new")
            throw MTPNGError.unknownError
        }
    }
    
    deinit {
        // handle can be cleared by .finish()
        if handle != nil {
            let ret = mtpng_encoder_release(&handle)
            if ret != MTPNG_RESULT_OK {
                print("Unexpected failure in mtpng_encoder_release")
            }
            if handle != nil {
                print("Invalid state after mtpng_encoder_release")
            }
        }
    }
    
    // Can throw MTPNGError.unknownError
    //
    public func writeHeader(header: MTPNGHeader) throws {
        if handle == nil {
            print("encoder is already finished")
            throw MTPNGError.unknownError
        }
        let ret = mtpng_encoder_write_header(handle, header.handle)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }

    // Can throw MTPNGError.unknownError
    //
    public func writeImageRows(bytes: Span<UInt8>) throws {
        if handle == nil {
            print("encoder is already finished")
            throw MTPNGError.unknownError
        }
        try bytes.withUnsafeBufferPointer { buffer in
            let ret = mtpng_encoder_write_image_rows(handle, buffer.baseAddress, bytes.count)
            if ret != MTPNG_RESULT_OK {
                print("failed in mtpng_encoder_write_image_rows")
                throw MTPNGError.unknownError
            }
        }
    }

    //
    // Can throw MTPNGError.unknownError
    //
    public func finish() throws {
        if handle == nil {
            print("encoder is already finished")
            throw MTPNGError.unknownError;
        }
        let ret = mtpng_encoder_finish(&handle)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
        if handle != nil {
            print("failed to release encoder in mtpng_finish");
            throw MTPNGError.unknownError;
        }
    }
}
