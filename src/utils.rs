use ::std::io;
use ::std::io::{Error, ErrorKind, Write};

pub type IoResult = io::Result<()>;

pub fn invalid_input(payload: &str) -> Error
{
    Error::new(ErrorKind::InvalidInput, payload)
}

pub fn other(payload: &str) -> Error
{
    Error::new(ErrorKind::Other, payload)
}

pub fn write_be32<W: Write>(w: &mut W, val: u32) -> IoResult {
    let bytes = [
        (val >> 24 & 0xff) as u8,
        (val >> 16 & 0xff) as u8,
        (val >> 8 & 0xff) as u8,
        (val & 0xff) as u8,
    ];
    w.write_all(&bytes)
}

pub fn write_be16<W: Write>(w: &mut W, val: u16) -> IoResult {
    let bytes = [
        (val >> 8 & 0xff) as u8,
        (val & 0xff) as u8,
    ];
    w.write_all(&bytes)
}

pub fn write_byte<W: Write>(w: &mut W, val: u8) -> IoResult {
    let bytes = [val];
    w.write_all(&bytes)
}
