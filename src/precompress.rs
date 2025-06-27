use std::{
    cmp::max,
    fs::File,
    mem::take,
    path::{Path, PathBuf},
    thread::{spawn, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossbeam::channel::{bounded, Receiver, Sender};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::encode::{Context, Quality};

#[derive(Debug, Clone, Copy, EnumIter)]
pub(crate) enum Algorithm {
    Brotli,
    Deflate,
    Gzip,
    Zstd,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = match self {
            Algorithm::Brotli => "brotli",
            Algorithm::Deflate => "deflate",
            Algorithm::Gzip => "gzip",
            Algorithm::Zstd => "zstd",
        };
        f.write_str(out)
    }
}

impl Algorithm {
    fn extension(self) -> &'static str {
        match self {
            Self::Brotli => ".br",
            Self::Deflate => ".zz",
            Self::Gzip => ".gz",
            Self::Zstd => ".zst",
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Algorithms {
    pub(crate) brotli: bool,
    pub(crate) deflate: bool,
    pub(crate) gzip: bool,
    pub(crate) zstd: bool,
}

impl Algorithms {
    pub(crate) fn all_enabled() -> Self {
        Self {
            brotli: true,
            deflate: true,
            gzip: true,
            zstd: true,
        }
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = Algorithm> {
        Algorithm::iter().filter(move |algorithm| self.is_enabled(*algorithm))
    }

    fn is_enabled(&self, algorithm: Algorithm) -> bool {
        match algorithm {
            Algorithm::Brotli => self.brotli,
            Algorithm::Deflate => self.deflate,
            Algorithm::Gzip => self.gzip,
            Algorithm::Zstd => self.zstd,
        }
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
    tx: Sender<Unit>,
    handles: Vec<JoinHandle<Stats>>,
    algorithms: Algorithms,
    extensions: Option<Vec<String>>,
}

type Unit = (Algorithm, PathBuf);

impl Compressor {
    pub(crate) fn new(
        threads: usize,
        min_size: u64,
        quality: Quality,
        algorithms: Algorithms,
        extensions: Option<Vec<String>>,
    ) -> Self {
        let cap = max(threads * 2, 128);
        let (tx, rx): (Sender<Unit>, Receiver<Unit>) = bounded(cap);

        let handles = (0..threads)
            .map(|_| {
                let rx = rx.clone();
                spawn(move || Compressor::worker(rx, min_size, quality))
            })
            .collect();

        Compressor {
            tx,
            handles,
            algorithms,
            extensions,
        }
    }

    pub(crate) fn precompress(&self, path: &Path) {
        let walk = ignore::WalkBuilder::new(path)
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
                    eprintln!("Warning: {err}");
                    continue;
                }
            };
            let path = entry.path();
            if self.should_compress(path) && !path.is_symlink() && path.is_file() {
                for alg in self.algorithms.iter() {
                    let path = path.to_path_buf();
                    self.tx
                        .send((alg, path))
                        .expect("unable to send on channel");
                }
            }
        }
    }

    pub(crate) fn finish(mut self) -> Stats {
        let handles = take(&mut self.handles);
        drop(self);

        handles.into_iter().fold(Stats::default(), |stats, handle| {
            stats + handle.join().expect("unable to join worker thread")
        })
    }

    fn worker(rx: Receiver<Unit>, min_size: u64, quality: Quality) -> Stats {
        let mut stats = Stats::default();
        let mut ctx = Context::new(1 << 14, quality);

        while let Ok((algorithm, pathbuf)) = rx.recv() {
            let start = Instant::now();
            match Compressor::encode_file(&mut ctx, min_size, algorithm, &pathbuf) {
                Err(err) => {
                    eprintln!("Warning: {}: {}", pathbuf.display(), err);
                    stats.num_errors += 1;
                }
                Ok(Some((src, dst))) => {
                    let dur = start.elapsed();
                    let s = match algorithm {
                        Algorithm::Brotli => &mut stats.brotli,
                        Algorithm::Deflate => &mut stats.deflate,
                        Algorithm::Gzip => &mut stats.gzip,
                        Algorithm::Zstd => &mut stats.zstd,
                    };
                    s.total_time += dur;
                    s.saved_bytes += (src - dst) as i64;
                    s.total_bytes += dst;
                    stats.num_files += 1;
                }
                Ok(None) => {}
            }
        }

        stats
    }

    fn encode_file(
        ctx: &mut Context,
        min_size: u64,
        alg: Algorithm,
        path: &Path,
    ) -> Result<Option<(u64, u64)>> {
        let mut src = File::open(path)?;
        let src_size = src.metadata()?.len();
        if src_size < min_size {
            return Ok(None);
        }

        let mut file_name = match path.file_name() {
            None => return Ok(None),
            Some(name) => name.to_os_string(),
        };
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
        Ok(Some((src_size, dst_size)))
    }

    fn should_compress(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext) = ext.to_str() {
                return if let Some(exts) = &self.extensions {
                    exts.iter().any(|v| v == ext)
                } else {
                    EXTENSIONS.contains(ext)
                };
            }
        }
        false
    }
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
