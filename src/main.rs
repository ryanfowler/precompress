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

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
    let args = Args::parse();
    let threads = match args.threads {
        0 => num_cpus::get(),
        t => t,
    };
    let quality = Quality {
        brotli: args.brotli_quality,
        deflate: args.deflate_quality,
        gzip: args.gzip_quality,
        zstd: args.zstd_quality,
    };
    let algs = Algorithms {
        brotli: args.brotli,
        deflate: args.deflate,
        gzip: args.gzip,
        zstd: args.zstd,
    };

    if algs.iter().count() == 0 {
        eprintln!("Error: no compression algorithms enabled");
        exit(1);
    }

    let cmp = Compressor::new(threads, quality, algs);
    let start = Instant::now();
    let stats = cmp.precompress(&args.path);
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

    /// Enable brotli compression.
    #[clap(long, action, default_missing_value = "true")]
    brotli: bool,

    /// Enable deflate compression.
    #[clap(long, action, default_missing_value = "true")]
    deflate: bool,

    /// Enable gzip compression.
    #[clap(long, action, default_missing_value = "true")]
    gzip: bool,

    /// Enable zstd compression.
    #[clap(long, action, default_missing_value = "true")]
    zstd: bool,

    /// Set brotli compression quality.
    #[clap(long, default_value = "11")]
    brotli_quality: u8,

    /// Set deflate compression quality.
    #[clap(long, default_value = "9")]
    deflate_quality: u8,

    /// Set gzip compression quality.
    #[clap(long, default_value = "9")]
    gzip_quality: u8,

    /// Set zstd compression quality.
    #[clap(long, default_value = "21")]
    zstd_quality: u8,

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
