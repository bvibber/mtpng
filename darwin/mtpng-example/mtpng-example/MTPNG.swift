//
//  MTPNG.swift
//  mtpng-example
//
//  Created by Brooke on 6/15/26.
//  Copyright © 2026 Brooke Vibber. All rights reserved.
//

import Foundation
import mtpng

enum MTPNGError: Error {
    case unknownError
    case inputOutOfBounds
}

enum MTPNGColor: UInt32 {
    case Greyscale = 0 // MTPNG_COLOR_GREYSCALE
    case Truecolor = 2 // MTPNG_COLOR_TRUECOLOR
    case IndexedColor = 3 // MTPNG_COLOR_INDEXED_COLOR
    case GreyscaleAlpha = 4 // MTPNG_COLOR_GREYSCALE_ALPHA
    case TruecolorAlpha = 6 // MTPNG_COLOR_TRUECOLOR_ALPHA
}

typealias MTPNGWriteFunc = (_: Span<UInt8>) -> Int

typealias MTPNGFlushFunc = () -> Bool

class MTPNGThreadPool {
    var handle: OpaquePointer? = nil

    init(threads: Int) throws {
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

class MTPNGEncoderOptions {
    var handle: OpaquePointer? = nil

    init() throws {
        let ret = mtpng_encoder_options_new(&self.handle)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
        if self.handle == nil {
            throw MTPNGError.unknownError
        }
    }

    deinit {
        let ret = mtpng_encoder_options_release(&self.handle)
        if (ret != MTPNG_RESULT_OK) {
            print("Unexpected failure in mtpng_encoder_options_release")
        }
        if self.handle != nil {
            print("Invalid state after mtpng_encoder_options_release")
        }

    }
    
    func setThreadPool(pool: MTPNGThreadPool) throws {
        let ret = mtpng_encoder_options_set_thread_pool(self.handle, pool.handle)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }

    // mtpng_encoder_options_set_filter
    // mtpng_encoder_options_set_strategy
    // mtpng_encoder_options_set_compression_level
    // mtpng_encoder_options_set_chunk_size
}

class MTPNGHeader {
    var handle: OpaquePointer? = nil
    
    init () throws {
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
    
    func setSize(width: UInt32, height: UInt32) throws {
        let ret = mtpng_header_set_size(self.handle, width, height)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.inputOutOfBounds
        }
    }

    func setColor(color: MTPNGColor, bits: UInt8) throws {
        let ret = mtpng_header_set_color(self.handle, mtpng_color_t(color.rawValue), bits)
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

class MTPNGEncoder {
    var handle: OpaquePointer? = nil
    var writeFunc: MTPNGWriteFunc? = nil
    var flushFunc: MTPNGFlushFunc? = nil

    init (write: MTPNGWriteFunc?, flush: MTPNGFlushFunc?, options: MTPNGEncoderOptions) throws {
        self.writeFunc = write
        self.flushFunc = flush
        let user_data = UnsafeMutableRawPointer.init(Unmanaged.passUnretained(self).toOpaque());
        let ret = mtpng_encoder_new(&handle,
                          write_func,
                          flush_func,
                          user_data,
                          options.handle)
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
    
    func writeHeader(header: MTPNGHeader) throws {
        if handle == nil {
            print("encoder is already finished")
            throw MTPNGError.unknownError
        }
        let ret = mtpng_encoder_write_header(handle, header.handle)
        if ret != MTPNG_RESULT_OK {
            throw MTPNGError.unknownError
        }
    }
    
    func writeImageRows(bytes: Span<UInt8>) throws {
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
    
    func finish() throws {
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
