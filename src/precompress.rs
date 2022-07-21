use std::{
    cmp::max,
    fs::File,
    path::{Path, PathBuf},
    thread::spawn,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossbeam::channel::{bounded, Receiver, Sender};

use crate::encode::{Context, Quality};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Algorithm {
    Brotli,
    Deflate,
    Gzip,
    Zstd,
}

impl Algorithm {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Brotli => "brotli",
            Self::Deflate => "deflate",
            Self::Gzip => "gzip",
            Self::Zstd => "zstd",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Brotli => ".br",
            Self::Deflate => ".zz",
            Self::Gzip => ".gz",
            Self::Zstd => ".zst",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Algorithms {
    pub(crate) brotli: bool,
    pub(crate) deflate: bool,
    pub(crate) gzip: bool,
    pub(crate) zstd: bool,
}

impl Algorithms {
    pub(crate) fn enabled(&self) -> Vec<Algorithm> {
        let mut out = Vec::with_capacity(4);
        if self.brotli {
            out.push(Algorithm::Brotli);
        }
        if self.deflate {
            out.push(Algorithm::Deflate);
        }
        if self.gzip {
            out.push(Algorithm::Gzip);
        }
        if self.zstd {
            out.push(Algorithm::Zstd);
        }
        out
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Stats {
    pub(crate) num_files: u64,
    pub(crate) num_errors: u64,

    pub(crate) brotli: AlgStat,
    pub(crate) deflate: AlgStat,
    pub(crate) gzip: AlgStat,
    pub(crate) zstd: AlgStat,
}

impl Stats {
    pub(crate) fn for_algorithm(&self, alg: Algorithm) -> AlgStat {
        match alg {
            Algorithm::Brotli => self.brotli,
            Algorithm::Deflate => self.deflate,
            Algorithm::Gzip => self.gzip,
            Algorithm::Zstd => self.zstd,
        }
    }
}

impl std::ops::Add<Stats> for Stats {
    type Output = Stats;

    fn add(self, rhs: Stats) -> Stats {
        Stats {
            num_files: self.num_files + rhs.num_files,
            num_errors: self.num_errors + rhs.num_errors,
            brotli: self.brotli + rhs.brotli,
            deflate: self.deflate + rhs.deflate,
            gzip: self.gzip + rhs.gzip,
            zstd: self.zstd + rhs.zstd,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AlgStat {
    pub(crate) total_time: Duration,
    pub(crate) total_bytes: u64,
    pub(crate) saved_bytes: i64,
}

impl std::ops::Add<AlgStat> for AlgStat {
    type Output = AlgStat;

    fn add(self, rhs: AlgStat) -> Self::Output {
        AlgStat {
            total_time: self.total_time + rhs.total_time,
            total_bytes: self.total_bytes + rhs.total_bytes,
            saved_bytes: self.saved_bytes + rhs.saved_bytes,
        }
    }
}

pub(crate) struct Compressor {
    threads: usize,
    quality: Quality,
    algorithms: Algorithms,
}

type Unit = (Algorithm, PathBuf);

impl Compressor {
    pub(crate) fn new(threads: usize, quality: Quality, algorithms: Algorithms) -> Self {
        Compressor {
            threads,
            quality,
            algorithms,
        }
    }

    pub(crate) fn precompress(&self, path: PathBuf) -> Stats {
        let cap = max(self.threads * 2, 64);
        let (tx, rx): (Sender<Unit>, Receiver<Unit>) = bounded(cap);

        let quality = self.quality;
        let mut handles = Vec::with_capacity(self.threads);
        for _ in 0..self.threads {
            let rx = rx.clone();
            handles.push(spawn(move || Compressor::worker(rx, quality)));
        }

        let enabled = self.algorithms.enabled();
        let walk = ignore::WalkBuilder::new(&path)
            .ignore(false)
            .git_exclude(false)
            .git_global(false)
            .git_ignore(false)
            .follow_links(false)
            .build();
        for entry in walk {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    eprintln!("Warning: {}", err);
                    continue;
                }
            };
            let path = entry.path();
            if should_compress(path) && !path.is_symlink() && path.is_file() {
                for alg in &enabled {
                    let path = path.to_path_buf();
                    tx.send((*alg, path)).expect("unable to send on channel")
                }
            }
        }
        drop(tx);

        let mut stats = Stats::default();
        for handle in handles {
            let h_stats = handle.join().expect("unable to join worker thread");
            stats = stats + h_stats;
        }
        stats
    }

    fn worker(rx: Receiver<Unit>, quality: Quality) -> Stats {
        let mut stats = Stats::default();
        let mut ctx = Context::new(1 << 14, quality);

        while let Ok((algorithm, pathbuf)) = rx.recv() {
            let start = Instant::now();
            match Compressor::encode_file(&mut ctx, algorithm, &pathbuf) {
                Err(err) => {
                    eprintln!("Warning: {}: {}", pathbuf.display(), err);
                    stats.num_errors += 1;
                }
                Ok((src, dst)) => {
                    let dur = start.elapsed();
                    match algorithm {
                        Algorithm::Brotli => {
                            stats.brotli.total_time += dur;
                            stats.brotli.saved_bytes += (src - dst) as i64;
                            stats.brotli.total_bytes += dst;
                        }
                        Algorithm::Deflate => {
                            stats.deflate.total_time += dur;
                            stats.deflate.saved_bytes += (src - dst) as i64;
                            stats.deflate.total_bytes += dst;
                        }
                        Algorithm::Gzip => {
                            stats.gzip.total_time += dur;
                            stats.gzip.saved_bytes += (src - dst) as i64;
                            stats.gzip.total_bytes += dst;
                        }
                        Algorithm::Zstd => {
                            stats.zstd.total_time += dur;
                            stats.zstd.saved_bytes += (src - dst) as i64;
                            stats.zstd.total_bytes += dst;
                        }
                    }
                    stats.num_files += 1;
                }
            }
        }

        stats
    }

    fn encode_file(ctx: &mut Context, alg: Algorithm, path: &PathBuf) -> Result<(u64, u64)> {
        let mut src = File::open(path)?;
        let src_size = src.metadata()?.len();

        let mut file_name = match path.file_name() {
            None => return Ok((0, 0)),
            Some(name) => name,
        }
        .to_os_string();
        file_name.push(alg.extension());
        let dst_path = path.with_file_name(file_name);

        let mut dst = File::create(dst_path)?;
        match alg {
            Algorithm::Brotli => ctx.write_brotli(&mut src, &mut dst)?,
            Algorithm::Deflate => ctx.write_deflate(&mut src, &mut dst)?,
            Algorithm::Gzip => ctx.write_gzip(&mut src, &mut dst)?,
            Algorithm::Zstd => ctx.write_zstd(&mut src, &mut dst)?,
        };
        let dst_size = dst.metadata()?.len();
        Ok((src_size, dst_size))
    }
}

fn should_compress(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        if let Some(ext) = ext.to_str() {
            return EXTENSIONS.contains(ext);
        }
    }
    false
}

static EXTENSIONS: phf::Set<&'static str> = phf::phf_set! {
    "atom",
    "conf",
    "css",
    "eot",
    "htm",
    "html",
    "js",
    "json",
    "jsx",
    "md",
    "otf",
    "rss",
    "scss",
    "sitemap",
    "svg",
    "text",
    "ts",
    "tsx",
    "ttf",
    "txt",
    "wasm",
    "xml",
    "yaml",
};
