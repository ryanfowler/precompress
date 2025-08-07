#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::exit;
use std::time::{Duration, Instant};

use clap::Parser;
use mimalloc::MiMalloc;
use precompress::Algorithm;

use crate::encode::Quality;
use crate::precompress::{Algorithms, Compressor, Stats};

mod encode;
mod precompress;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let args = Args::parse();
    let threads = match args.threads {
        0 => num_cpus::get(),
        t => t,
    };

    let (algs, quality) = parse_compression(args.compression);

    if algs.iter().count() == 0 {
        eprintln!("Error: no compression algorithms enabled");
        exit(1);
    }

    let exts = args.extensions.map(|v| {
        v.into_iter()
            .flat_map(|s| s.split(',').map(|s| s.to_owned()).collect::<Vec<_>>())
            .collect::<Vec<String>>()
    });

    let cmp = Compressor::new(threads, args.min_size, quality, algs, exts);
    let start = Instant::now();
    cmp.precompress(&args.path);
    let stats = cmp.finish();
    let took = start.elapsed();

    println!(
        "Compressed {} files in {}",
        stats.num_files,
        format_duration(took)
    );
    println!("Data compression:");
    for alg in algs.iter() {
        print_alg_savings(alg, &stats);
    }
}

/// Precompress a directory of static files.
#[derive(Parser, Debug)]
#[clap(version, about)]
struct Args {
    /// Directory to recursively compress files in.
    path: PathBuf,

    /// Compression algorithms to use.
    #[clap(short, long)]
    compression: Option<Vec<String>>,

    /// Extensions of files that should be compressed.
    #[clap(short, long)]
    extensions: Option<Vec<String>>,

    /// Set the minimum size of files to be compressed in bytes.
    #[clap(short, long, default_value = "1024")]
    min_size: u64,

    /// Number of threads to use; "0" uses the number of cpus.
    #[clap(short, long, default_value = "0")]
    threads: usize,
}

fn parse_compression(compression: Option<Vec<String>>) -> (Algorithms, Quality) {
    let mut quality = Quality::default();

    let algs = compression
        .map(|v| {
            let raw = v
                .into_iter()
                .flat_map(|s| s.split(',').map(|s| s.to_owned()).collect::<Vec<_>>());
            let mut algs = Algorithms::default();
            for s in raw {
                let (c, q) = if let Some((c, q)) = s.split_once(':') {
                    let q: u8 = match q.parse() {
                        Ok(q) => q,
                        Err(_) => {
                            eprintln!("Error: invalid compression quality: {q}");
                            exit(1);
                        }
                    };
                    (c, Some(q))
                } else {
                    (s.as_ref(), None)
                };

                match c {
                    "br" | "brotli" => {
                        algs.brotli = true;
                        if let Some(q) = q {
                            quality.brotli = q;
                        }
                    }
                    "de" | "deflate" => {
                        algs.deflate = true;
                        if let Some(q) = q {
                            quality.deflate = q;
                        }
                    }
                    "gz" | "gzip" => {
                        algs.gzip = true;
                        if let Some(q) = q {
                            quality.gzip = q;
                        }
                    }
                    "zstd" => {
                        algs.zstd = true;
                        if let Some(q) = q {
                            quality.zstd = q;
                        }
                    }
                    _ => {
                        eprintln!("Error: unknown compression algorithm: {s}");
                        exit(1);
                    }
                }
            }
            algs
        })
        .unwrap_or_else(Algorithms::all_enabled);

    (algs, quality)
}

fn print_alg_savings(alg: Algorithm, stats: &Stats) {
    let stat = stats.for_algorithm(alg);
    println!(
        "  {}: {}%",
        alg,
        calc_savings(stat.saved_bytes, stat.total_bytes),
    );
}

fn calc_savings(saved: i64, total: u64) -> u8 {
    ((saved as f64 / (saved as f64 + total as f64)) * 100.0) as u8
}

fn format_duration(dur: Duration) -> String {
    if dur.as_millis() < 1_000 {
        format!("{}ms", dur.as_millis())
    } else {
        format!("{}s", dur.as_secs())
    }
}
