//
// mtpng - a multithreaded parallel PNG encoder in Rust
// mtpng.rs - CLI utility for testing and Rust API example
//
// Copyright (c) 2018 Brion Vibber
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

use std::fs::File;
use std::io;
use std::io::{Error, ErrorKind};

// CLI options
extern crate clap;
use clap::{Arg, App, ArgMatches};

// For reading an existing file
extern crate png;

extern crate rayon;
use rayon::{ThreadPool, ThreadPoolBuilder};

// For timing!
extern crate time;
use time::precise_time_s;

// Hey that's us!
extern crate mtpng;
use mtpng::{ColorType, CompressionLevel, Header};
use mtpng::Mode::{Adaptive, Fixed};
use mtpng::encoder::Encoder;
use mtpng::deflate::Strategy;
use mtpng::filter::Filter;

pub fn err(payload: &str) -> Error
{
    Error::new(ErrorKind::Other, payload)
}

fn read_png(filename: &str) -> io::Result<(Header, Vec<u8>)> {
    let decoder = png::Decoder::new(File::open(filename)?);
    let (info, mut reader) = decoder.read_info()?;

    let header = Header::with_depth(info.width,
                                    info.height,
                                    ColorType::from_u8(info.color_type as u8)?,
                                    info.bit_depth as u8);
    let mut data = vec![0u8; info.buffer_size()];
    reader.next_frame(&mut data)?;

    Ok((header, data))
}

fn write_png(pool: &ThreadPool, args: &ArgMatches,
             filename: &str, header: Header, data: &[u8])
   -> io::Result<()>
{
    let writer = try!(File::create(filename));

    let mut encoder = Encoder::with_thread_pool(writer, pool);

    // Encoding options
    match args.value_of("chunk-size") {
        None    => {},
        Some(s) => {
            let n = s.parse::<usize>().unwrap();
            encoder.set_chunk_size(n)?;
        },
    }

    match args.value_of("filter") {
        None             => {},
        Some("adaptive") => encoder.set_filter_mode(Adaptive)?,
        Some("none")     => encoder.set_filter_mode(Fixed(Filter::None))?,
        Some("up")       => encoder.set_filter_mode(Fixed(Filter::Up))?,
        Some("sub")      => encoder.set_filter_mode(Fixed(Filter::Sub))?,
        Some("average")  => encoder.set_filter_mode(Fixed(Filter::Average))?,
        Some("paeth")    => encoder.set_filter_mode(Fixed(Filter::Paeth))?,
        _                => return Err(err("Unsupported filter type")),
    }

    match args.value_of("level") {
        None            => {},
        Some("default") => encoder.set_compression_level(CompressionLevel::Default)?,
        Some("1")       => encoder.set_compression_level(CompressionLevel::Fast)?,
        Some("9")       => encoder.set_compression_level(CompressionLevel::High)?,
        _               => return Err(err("Unsupported compression level (try default, 1, or 9)")),
    }

    match args.value_of("strategy") {
        None             => {},
        Some("auto")     => encoder.set_strategy_mode(Adaptive)?,
        Some("default")  => encoder.set_strategy_mode(Fixed(Strategy::Default))?,
        Some("filtered") => encoder.set_strategy_mode(Fixed(Strategy::Filtered))?,
        Some("huffman")  => encoder.set_strategy_mode(Fixed(Strategy::HuffmanOnly))?,
        Some("rle")      => encoder.set_strategy_mode(Fixed(Strategy::RLE))?,
        Some("fixed")    => encoder.set_strategy_mode(Fixed(Strategy::Fixed))?,
        _                => return Err(err("Invalid compression strategy mode"))?,
    }

    // Image data
    encoder.set_size(header.width, header.height)?;
    encoder.set_color(header.color_type, header.depth)?;
    encoder.write_header()?;

    for i in 0 .. header.height as usize {
        let start = header.stride() * i;
        let end = start + header.stride();
        encoder.append_row(&data[start .. end])?;
    }
    encoder.finish()?;

    Ok(())
}

fn doit(args: ArgMatches) -> io::Result<()> {
    let threads = match args.value_of("threads") {
        None    => 0, // Means default
        Some(s) => {
            s.parse::<usize>().unwrap()
        },
    };

    let pool = ThreadPoolBuilder::new().num_threads(threads)
                                       .build()
                                       .unwrap();
    eprintln!("Using {} threads", pool.current_num_threads());

    let reps = match args.value_of("repeat") {
        Some(s) => {
            s.parse::<usize>().unwrap()
        },
        None => 1,
    };

    let infile = args.value_of("input").unwrap();
    let outfile = args.value_of("output").unwrap();

    println!("{} -> {}", infile, outfile);
    let (header, data) = read_png(&infile)?;

    for _i in 0 .. reps {
        let start_time = precise_time_s();
        write_png(&pool, &args, &outfile, header, &data)?;
        let delta = precise_time_s() - start_time;

        println!("Done in {} ms", (delta * 1000.0).round());
    }

    Ok(())
}

pub fn main() {
    let matches = App::new("mtpng parallel PNG encoder")
        .version("0.1.0")
        .author("Brion Vibber <brion@pobox.com>")
        .about("Re-encodes PNG images using multiple CPU cores to exercise the mtpng library.")
        .arg(Arg::with_name("chunk-size")
            .long("chunk-size")
            .value_name("bytes")
            .help("Divide image into chunks of at least this given size.")
            .takes_value(true))
        .arg(Arg::with_name("filter")
            .long("filter")
            .value_name("filter")
            .help("Set a fixed filter: one of none, sub, up, average, or paeth."))
        .arg(Arg::with_name("level")
            .long("level")
            .value_name("level")
            .help("Set deflate compression level, from 1-9."))
        .arg(Arg::with_name("strategy")
            .long("strategy")
            .value_name("strategy")
            .help("Deflate strategy: one of filtered, huffman, rle, or fixed."))
        .arg(Arg::with_name("threads")
            .long("threads")
            .value_name("threads")
            .help("Override default number of threads."))
        .arg(Arg::with_name("repeat")
            .long("repeat")
            .value_name("n")
            .help("Run conversion n times, as load benchmarking helper."))
        .arg(Arg::with_name("input")
            .help("Input filename, must be another PNG.")
            .required(true)
            .index(1))
        .arg(Arg::with_name("output")
            .help("Output filename.")
            .required(true)
            .index(2))
        .get_matches();

    match doit(matches) {
        Ok(()) => {},
        Err(e) => eprintln!("Error: {}", e),
    }
}