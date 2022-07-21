use std::path::PathBuf;
use std::process::exit;
use std::time::{Duration, Instant};

use clap::Parser;

use crate::encode::Quality;
use crate::precompress::{AlgStat, Algorithms, Compressor};

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
    let quality = Quality {
        brotli: None,
        deflate: None,
        gzip: None,
        zstd: None,
    };
    let algs = Algorithms {
        brotli: args.brotli.unwrap_or(false),
        deflate: args.deflate.unwrap_or(false),
        gzip: args.gzip.unwrap_or(false),
        zstd: args.zstd.unwrap_or(false),
    };

    if !algs.brotli && !algs.deflate && !algs.gzip && !algs.zstd {
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
    for alg in algs.enabled() {
        print_alg_savings(alg.name(), stats.for_algorithm(alg));
    }
}

fn print_alg_savings(alg: &str, stat: AlgStat) {
    println!(
        "  {}: {}%",
        alg,
        calculate_savings(stat.saved_bytes, stat.total_bytes)
    );
}

fn calculate_savings(saved: i64, total: u64) -> u8 {
    ((saved as f64 / (saved as f64 + total as f64)) * 100.0) as u8
}

fn format_duration(dur: Duration) -> String {
    if dur.as_millis() < 1_000 {
        format!("{}ms", dur.as_millis())
    } else {
        format!("{}s", dur.as_secs())
    }
}
