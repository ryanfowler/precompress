use std::path::PathBuf;
use std::process::exit;

use clap::Parser;

use crate::encode::Quality;
use crate::precompress::{Algorithms, Compressor};

mod encode;
mod precompress;

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
    let stats = cmp.precompress(args.path);

    println!("Compressed {} files", stats.num_files);
    if algs.brotli {
        println!("brotli: {:?}", stats.brotli_time);
    }
    if algs.deflate {
        println!("deflate: {:?}", stats.deflate_time);
    }
    if algs.gzip {
        println!("gzip: {:?}", stats.gzip_time);
    }
    if algs.zstd {
        println!("zstd: {:?}", stats.zstd_time);
    }
}
