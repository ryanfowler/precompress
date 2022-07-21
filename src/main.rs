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

/// Precompress a directory of static files.
#[derive(Parser, Debug)]
#[clap(version, about)]
struct Args {
    /// Directory to recursively compress files in.
    path: PathBuf,

    /// Enable brotli compression.
    #[clap(long, action, takes_value = true, default_missing_value = "true")]
    brotli: Option<bool>,

    /// Enable deflate compression.
    #[clap(long, action, takes_value = true, default_missing_value = "true")]
    deflate: Option<bool>,

    /// Enable gzip compression.
    #[clap(long, action, takes_value = true, default_missing_value = "true")]
    gzip: Option<bool>,

    /// Enable zstd compression.
    #[clap(long, action, takes_value = true, default_missing_value = "true")]
    zstd: Option<bool>,

    /// Number of threads to use; "0" uses the number of cpus.
    #[clap(short, long, default_value = "0")]
    threads: usize,
}

fn main() {
    let args = Args::parse();
    let threads = match args.threads {
        0 => num_cpus::get(),
        t => t,
    };
    let quality = Quality::default();
    let algs = Algorithms::new(args.brotli, args.deflate, args.gzip, args.zstd);

    let algs_enabled = algs.enabled();
    if algs_enabled.is_empty() {
        eprintln!("Error: no compression algorithms selected");
        exit(1);
    }

    let cmp = Compressor::new(threads, quality, algs);
    let start = Instant::now();
    let stats = cmp.precompress(args.path);
    let took = start.elapsed();

    println!(
        "Compressed {} files in {}",
        stats.num_files,
        format_duration(took)
    );
    println!("Data compression:");
    for alg in algs_enabled {
        print_alg_savings(alg, &stats);
    }
}

fn print_alg_savings(alg: Algorithm, stats: &Stats) {
    let stat = stats.for_algorithm(alg);
    println!(
        "  {}: {}%",
        alg,
        calc_savings(stat.saved_bytes, stat.total_bytes)
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
