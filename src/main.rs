#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::exit;
use std::time::{Duration, Instant};

use clap::Parser;
use precompress::Algorithm;

use crate::encode::Quality;
use crate::precompress::{Algorithms, Compressor, Stats};

mod encode;
mod precompress;

#[cfg(not(target_os = "windows"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
    let args = Args::parse();
    let threads = match args.threads {
        0 => num_cpus::get(),
        t => t,
    };
    let quality = Quality {
        brotli: args.brotli.unwrap_or(None).unwrap_or(11),
        deflate: args.deflate.unwrap_or(None).unwrap_or(9),
        gzip: args.gzip.unwrap_or(None).unwrap_or(9),
        zstd: args.zstd.unwrap_or(None).unwrap_or(21),
    };
    let algs = Algorithms {
        brotli: args.brotli.is_some(),
        deflate: args.deflate.is_some(),
        gzip: args.gzip.is_some(),
        zstd: args.zstd.is_some(),
    };

    if algs.iter().count() == 0 {
        eprintln!("Error: no compression algorithms enabled");
        exit(1);
    }

    let cmp = Compressor::new(threads, args.min_size, quality, algs);
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

    /// Enable brotli compression with optional quality [default: 11]
    #[clap(long, value_name = "QUALITY")]
    brotli: Option<Option<u8>>,

    /// Enable deflate compression with optional quality [default: 9]
    #[clap(long, value_name = "QUALITY")]
    deflate: Option<Option<u8>>,

    /// Enable gzip compression with optional quality [default: 9]
    #[clap(long, value_name = "QUALITY")]
    gzip: Option<Option<u8>>,

    /// Enable zstd compression with optional quality [default: 21]
    #[clap(long, value_name = "QUALITY")]
    zstd: Option<Option<u8>>,

    /// Set the minimum size of files to be compressed in bytes.
    #[clap(long, default_value = "1024")]
    min_size: u64,

    /// Number of threads to use; "0" uses the number of cpus.
    #[clap(short, long, default_value = "0")]
    threads: usize,
}

fn print_alg_savings(alg: Algorithm, stats: &Stats) {
    let stat = stats.for_algorithm(alg);
    println!(
        "  {}: {}% ({} cpu time)",
        alg,
        calc_savings(stat.saved_bytes, stat.total_bytes),
        format_duration(stat.total_time),
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
