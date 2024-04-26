//
// mtpng - a multithreaded parallel PNG encoder in Rust
// writer.rs - low-level PNG chunk writer
//
// Copyright (c) 2018-2024 Brooke Vibber
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

use crc::crc32;
use crc::Hasher32;

use std::io;
use std::io::Write;

use super::Header;

use super::utils::*;

pub struct Writer<W: Write> {
    output: W,
}

impl<W: Write> Writer<W> {
    //
    // Creates a new PNG chunk stream writer.
    // Consumes the output Write object, but will
    // give it back to you via Writer::close().
    //
    pub fn new(output: W) -> Writer<W> {
        Writer {
            output,
        }
    }

    //
    // Close out the writer and return the Write
    // passed in originally so it can be used for
    // further output if necessary.
    //
    // Consumes the writer.
    //
    pub fn finish(mut self: Writer<W>) -> io::Result<W> {
        self.flush()?;
        Ok(self.output)
    }

    //
    // Write the PNG file signature to output stream.
    // https://www.w3.org/TR/PNG/#5PNG-file-signature
    //
    pub fn write_signature(&mut self) -> IoResult {
        let bytes = [
            137u8, // ???
            80u8,  // 'P'
            78u8,  // 'N'
            71u8,  // 'G'
            13u8,  // \r
            10u8,  // \n
            26u8,  // SUB
            10u8   // \n
        ];
        self.write_bytes(&bytes)
    }

    fn write_be32(&mut self, val: u32) -> IoResult {
        write_be32(&mut self.output, val)
    }

    fn write_bytes(&mut self, data: &[u8]) -> IoResult {
        self.output.write_all(data)
    }

    //
    // Write a chunk to the output stream.
    //
    // https://www.w3.org/TR/PNG/#5DataRep
    // https://www.w3.org/TR/PNG/#5CRC-algorithm
    //
    pub fn write_chunk(&mut self, tag: &[u8], data: &[u8]) -> IoResult {
        if tag.len() != 4 {
            return Err(invalid_input("Chunk tags must be 4 bytes"));
        }
        if data.len() > u32::max_value() as usize {
            return Err(invalid_input("Data chunks cannot exceed 4 GiB - 1 byte"));
        }

        // CRC covers both tag and data.
        let mut digest = crc32::Digest::new(crc32::IEEE);
        digest.write(tag);
        digest.write(data);
        let checksum = digest.sum32();

        // Write data...
        self.write_be32(data.len() as u32)?;
        self.write_bytes(tag)?;
        self.write_bytes(data)?;
        self.write_be32(checksum)
    }

    //
    // IHDR - first chunk in the file.
    // https://www.w3.org/TR/PNG/#11IHDR
    //
    pub fn write_header(&mut self, header: Header) -> IoResult {
        let mut data = Vec::<u8>::new();
        write_be32(&mut data, header.width)?;
        write_be32(&mut data, header.height)?;
        write_byte(&mut data, header.depth)?;
        write_byte(&mut data, header.color_type as u8)?;
        write_byte(&mut data, header.compression_method as u8)?;
        write_byte(&mut data, header.filter_method as u8)?;
        write_byte(&mut data, header.interlace_method as u8)?;

        self.write_chunk(b"IHDR", &data)
    }

    //
    // IEND - last chunk in the file.
    // https://www.w3.org/TR/PNG/#11IEND
    //
    pub fn write_end(&mut self) -> IoResult {
        self.write_chunk(b"IEND", b"")
    }

    //
    // Flush output.
    //
    pub fn flush(&mut self) -> IoResult {
        self.output.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::Writer;
    use super::IoResult;

    fn test_writer<F, G>(test_func: F, assert_func: G)
        where F: Fn(&mut Writer<Vec<u8>>) -> IoResult,
              G: Fn(&[u8])
    {
        let result = (|| -> io::Result<Vec<u8>> {
            let output = Vec::<u8>::new();
            let mut writer = Writer::new(output);
            test_func(&mut writer)?;
            writer.finish()
        })();
        match result {
            Ok(output) => assert_func(&output),
            Err(e) => assert!(false, "Error: {}", e),
        }
    }

    #[test]
    fn it_works() {
        test_writer(|_writer| {
            Ok(())
        }, |output| {
            assert_eq!(output.len(), 0);
        })
    }

    #[test]
    fn header_works() {
        test_writer(|writer| {
            writer.write_signature()
        }, |output| {
            assert_eq!(output.len(), 8);
        })
    }

    #[test]
    fn empty_chunk_works() {
        test_writer(|writer| {
            writer.write_chunk(b"IDAT", b"")
        }, |output| {
            // 4 bytes len
            // 4 bytes tag
            // 4 bytes crc
            assert_eq!(output.len(), 12);
        })
    }

    #[test]
    fn full_chunk_works() {
        test_writer(|writer| {
            writer.write_chunk(b"IDAT", b"01234567890123456789")
        }, |output| {
            // 4 bytes len
            // 4 bytes tag
            // 20 bytes data
            // 4 bytes crc
            assert_eq!(output.len(), 32);
        })
    }

    #[test]
    fn crc_works() {
        // From a 1x1 truecolor black pixel made with gd
        let one_pixel = b"\x08\x99\x63\x60\x60\x60\x00\x00\x00\x04\x00\x01";
        test_writer(|writer| {
            writer.write_chunk(b"IDAT", one_pixel)
        }, |output| {
            assert_eq!(output[0..4], b"\x00\x00\x00\x0c"[..], "expected length 12");
            assert_eq!(output[4..8], b"IDAT"[..], "expected IDAT");
            assert_eq!(output[8..20], one_pixel[..], "expected data payload");
            assert_eq!(output[20..24], b"\xa3\x0a\x15\xe3"[..], "expected crc32");
        })
    }
}
