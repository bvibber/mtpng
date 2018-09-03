use crc::crc32;
use crc::Hasher32;

use std::io;
use std::io::Write;

use super::Header;
use super::ColorType;

type IoResult = io::Result<()>;

fn write_be32<W: Write>(w: &mut W, val: u32) -> IoResult {
    let bytes = [
        (val >> 24 & 0xff) as u8,
        (val >> 16 & 0xff) as u8,
        (val >> 8 & 0xff) as u8,
        (val & 0xff) as u8,
    ];
    w.write_all(&bytes)
}

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
            output: output,
        }
    }

    //
    // Close out the writer and return the Write
    // passed in originally so it can be used for
    // further output if necessary.
    //
    // Consumes the writer.
    //
    pub fn close(mut this: Writer<W>) -> io::Result<W> {
        this.flush()?;
        Ok(this.output)
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
        self.output.write_all(&data)
    }

    //
    // Write a chunk to the output stream.
    //
    // https://www.w3.org/TR/PNG/#5CRC-algorithm
    //
    pub fn write_chunk(&mut self, tag: &[u8], data: &[u8]) -> IoResult {
        assert_eq!(tag.len(), 4);
        assert!(data.len() <= u32::max_value() as usize);

        // CRC is initialized to all 1 bits, and covers both tag and data.
        let mut digest = crc32::Digest::new_with_initial(crc32::IEEE, 0xffffffffu32);
        digest.write(tag);
        digest.write(data);
        let checksum = digest.sum32();

        self.write_be32(data.len() as u32)?;
        self.write_bytes(tag)?;
        self.write_bytes(data)?;
        self.write_be32(checksum)
    }

    //
    // https://www.w3.org/TR/PNG/#11IHDR
    //
    pub fn write_header(&mut self, header: Header) -> IoResult {
        // fixme
        Ok(())
    }

    pub fn flush(&mut self) -> IoResult {
        self.output.flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::Write;

    use super::Writer;
    use super::IoResult;

    fn test_writer<F, G>(test_func: F, assert_func: G)
        where F: Fn(&mut Writer<Vec<u8>>) -> IoResult,
              G: Fn(&[u8])
    {
        let result = (|| -> io::Result<Vec<u8>> {
            let mut output = Vec::<u8>::new();
            let mut writer = Writer::new(output);
            test_func(&mut writer)?;
            Writer::close(writer)
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
}
