use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::env;

mod lzss_stream;
use crate::lzss_stream::Lzss;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <compress|decompress> <input_file> [output_file]", args[0]);
        std::process::exit(1);
    }

    let command = &args[1];
    let input_file = &args[2];
    let output_file = if args.len() > 3 {
        args[3].clone()
    } else {
        match command.as_str() {
            "compress" => format!("{}.lzss", input_file),
            "decompress" => {
                if input_file.ends_with(".lzss") {
                    input_file.trim_end_matches(".lzss").to_string()
                } else {
                    format!("{}.decompressed", input_file)
                }
            }
            _ => {
                eprintln!("Invalid command. Use 'compress' or 'decompress'");
                std::process::exit(1);
            }
        }
    };

    match command.as_str() {
        "compress" => compress_file(input_file, &output_file),
        "decompress" => decompress_file(input_file, &output_file),
        _ => {
            eprintln!("Invalid command. Use 'compress' or 'decompress'");
            std::process::exit(1);
        }
    }
}

fn compress_file<P: AsRef<Path>>(input_path: P, output_path: P) -> io::Result<()> {
    // Read the entire input file into memory
    let mut input_file = File::open(&input_path)?;
    let mut input_data = Vec::new();
    input_file.read_to_end(&mut input_data)?;

    let input_size = input_data.len();
    println!("Reading file: {} bytes", input_size);

    // Compress the data
    let mut lzss = Lzss::new();
    let compressed_data = lzss.compress(&input_data)?;

    let compressed_size = compressed_data.len();
    println!("Compressed: {} bytes -> {} bytes ({:.1}% of original)", 
             input_size, compressed_size, 
             (compressed_size as f64 / input_size as f64) * 100.0);

    // Write compressed data to output file
    let mut output_file = File::create(&output_path)?;
    output_file.write_all(&compressed_data)?;
    output_file.flush()?;

    println!("Compressed to {}", output_path.as_ref().display());
    Ok(())
}

fn decompress_file<P: AsRef<Path>>(input_path: P, output_path: P) -> io::Result<()> {
    // Read the entire compressed file into memory
    let mut input_file = File::open(&input_path)?;
    let mut compressed_data = Vec::new();
    input_file.read_to_end(&mut compressed_data)?;

    let compressed_size = compressed_data.len();
    println!("Reading compressed file: {} bytes", compressed_size);

    // Decompress the data
    let mut lzss = Lzss::new();
    let decompressed_data = lzss.decompress(&compressed_data)?;

    let decompressed_size = decompressed_data.len();
    println!("Decompressed: {} bytes -> {} bytes", compressed_size, decompressed_size);

    // Write decompressed data to output file
    let mut output_file = File::create(&output_path)?;
    output_file.write_all(&decompressed_data)?;
    output_file.flush()?;

    println!("Decompressed to {}", output_path.as_ref().display());
    Ok(())
}
