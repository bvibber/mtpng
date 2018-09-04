// Hey that's us!
extern crate mtpng;

// For reading an existing file
extern crate png;

use std::env;
use std::fs::File;
use std::io;
use std::io::{Error, ErrorKind, Read, Write};

use mtpng::{Header, ColorType, Encoder, Options};

use png::Decoder;

pub fn err(payload: &str) -> Error
{
    Error::new(ErrorKind::Other, payload)
}

fn read_png(filename: &str) -> io::Result<(Header, Vec<u8>)> {
    let decoder = png::Decoder::new(File::open(filename)?);
    let (info, mut reader) = decoder.read_info()?;

    let mut header = Header::with_color(info.width, info.height, ColorType::Truecolor);
    let mut data = vec![0u8; info.buffer_size()];
    reader.next_frame(&mut data)?;

    Ok((header, data))
}

fn write_png(filename: &str, header: Header, data: &[u8]) -> io::Result<()> {
    let mut writer = try!(File::create(filename));
    let mut encoder = Encoder::new(header, Options::default(), writer);
    encoder.write_header()?;
    for i in 0 .. header.height as usize {
        let start = header.stride() * i;
        let end = start + header.stride();
        encoder.append_row(&data[start .. end])?;
    }
    encoder.flush()?;
    Ok(())
}

fn doit(infile: &str, outfile: &str) -> io::Result<()> {
    println!("{} -> {}", infile, outfile);
    let (header, data) = read_png(&infile)?;
    write_png(&outfile, header, &data)?;
    Ok(())
}

pub fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: mtpng <infile.png> <outfile.png>");
    } else {
        let infile = &args[1];
        let outfile = &args[2];
        match doit(&infile, &outfile) {
            Ok(()) => {},
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}