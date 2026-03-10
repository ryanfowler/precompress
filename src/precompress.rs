use std::{
    cmp::max,
    collections::HashSet,
    fs::{self, File},
    io,
    mem::take,
    path::{Path, PathBuf},
    thread::{JoinHandle, spawn},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use ignore::overrides::OverrideBuilder;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::encode::{Context, Quality};
use crate::{calc_savings, format_bytes};

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

#[derive(Debug, Clone, Copy)]
pub(crate) struct Algorithms {
    pub(crate) brotli: bool,
    pub(crate) deflate: bool,
    pub(crate) gzip: bool,
    pub(crate) zstd: bool,
}

impl Default for Algorithms {
    fn default() -> Self {
        Self {
            brotli: true,
            deflate: false,
            gzip: true,
            zstd: true,
        }
    }
}

impl Algorithms {
    pub(crate) fn empty() -> Self {
        Self {
            brotli: false,
            deflate: false,
            gzip: false,
            zstd: false,
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
    extensions: Option<HashSet<String>>,
    #[allow(dead_code)]
    verbose: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct WalkOptions {
    pub(crate) respect_ignore: bool,
    pub(crate) exclude: Vec<String>,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            respect_ignore: true,
            exclude: Vec::new(),
        }
    }
}

type Unit = (Algorithm, PathBuf);

impl Compressor {
    pub(crate) fn new(
        threads: usize,
        min_size: u64,
        quality: Quality,
        algorithms: Algorithms,
        extensions: Option<HashSet<String>>,
        verbose: bool,
    ) -> Self {
        let cap = max(threads * 2, 128);
        let (tx, rx): (Sender<Unit>, Receiver<Unit>) = bounded(cap);

        let handles = (0..threads)
            .map(|_| {
                let rx = rx.clone();
                spawn(move || Compressor::worker(rx, min_size, quality, verbose))
            })
            .collect();

        Compressor {
            tx,
            handles,
            algorithms,
            extensions,
            verbose,
        }
    }

    pub(crate) fn precompress(&self, path: &Path, walk_options: &WalkOptions) -> Result<()> {
        let walk = build_walk(path, walk_options)?;
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
                let path = path.to_path_buf();
                let algs: Vec<_> = self.algorithms.iter().collect();
                let (last, rest) = algs.split_last().unwrap();
                for alg in rest {
                    self.tx
                        .send((*alg, path.clone()))
                        .expect("unable to send on channel");
                }
                self.tx
                    .send((*last, path))
                    .expect("unable to send on channel");
            }
        }

        Ok(())
    }

    pub(crate) fn finish(mut self) -> Stats {
        let handles = take(&mut self.handles);
        drop(self);

        handles.into_iter().fold(Stats::default(), |stats, handle| {
            stats + handle.join().expect("unable to join worker thread")
        })
    }

    fn worker(rx: Receiver<Unit>, min_size: u64, quality: Quality, verbose: bool) -> Stats {
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
                    let saved = src as i64 - dst as i64;
                    if verbose {
                        let sign = if saved < 0 { "-" } else { "" };
                        eprintln!(
                            "{}: {} ({}%, {}{})",
                            algorithm,
                            pathbuf.display(),
                            calc_savings(saved, dst),
                            sign,
                            format_bytes(saved.unsigned_abs()),
                        );
                    }
                    let s = match algorithm {
                        Algorithm::Brotli => &mut stats.brotli,
                        Algorithm::Deflate => &mut stats.deflate,
                        Algorithm::Gzip => &mut stats.gzip,
                        Algorithm::Zstd => &mut stats.zstd,
                    };
                    s.total_time += dur;
                    s.saved_bytes += saved;
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

        let dst_size = write_atomic(&dst_path, |dst| match alg {
            Algorithm::Brotli => ctx.write_brotli(&mut src, dst),
            Algorithm::Deflate => ctx.write_deflate(&mut src, dst),
            Algorithm::Gzip => ctx.write_gzip(&mut src, dst),
            Algorithm::Zstd => ctx.write_zstd(&mut src, dst),
        })?;
        Ok(Some((src_size, dst_size)))
    }

    fn should_compress(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension()
            && let Some(ext) = ext.to_str()
        {
            return if let Some(exts) = &self.extensions {
                exts.contains(ext)
            } else {
                EXTENSIONS.contains(ext)
            };
        }
        false
    }
}

fn write_atomic(
    dst_path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<u64> {
    let tmp_path = tmp_output_path(dst_path);
    let result = (|| {
        let mut dst = File::create(&tmp_path)?;
        write(&mut dst)?;
        dst.sync_all()?;
        let dst_size = dst.metadata()?.len();
        drop(dst);
        fs::rename(&tmp_path, dst_path)?;
        Ok(dst_size)
    })();

    if result.is_err() {
        _ = fs::remove_file(&tmp_path);
    }

    result
}

fn tmp_output_path(dst_path: &Path) -> PathBuf {
    let mut file_name = dst_path.file_name().unwrap_or_default().to_os_string();
    file_name.push(".tmp");
    dst_path.with_file_name(file_name)
}

fn build_walk(path: &Path, walk_options: &WalkOptions) -> Result<ignore::Walk> {
    let mut builder = ignore::WalkBuilder::new(path);
    builder.follow_links(false);
    builder.require_git(false);

    if !walk_options.respect_ignore {
        builder
            .parents(false)
            .ignore(false)
            .git_exclude(false)
            .git_global(false)
            .git_ignore(false);
    }

    if !walk_options.exclude.is_empty() {
        let mut overrides = OverrideBuilder::new(path);
        for pattern in &walk_options.exclude {
            overrides.add(&format!("!{pattern}"))?;
        }
        builder.overrides(overrides.build()?);
    }

    Ok(builder.build())
}

static EXTENSIONS: phf::Set<&'static str> = phf::phf_set! {
    "atom",
    "cfg",
    "component",
    "conf",
    "css",
    "csv",
    "eot",
    "geojson",
    "graphql",
    "htm",
    "html",
    "ico",
    "java",
    "js",
    "json",
    "jsx",
    "ldjson",
    "log",
    "manifest",
    "map",
    "md",
    "mjs",
    "otf",
    "rss",
    "rtf",
    "scss",
    "sfnt",
    "sitemap",
    "svg",
    "text",
    "ts",
    "tsv",
    "tsx",
    "ttf",
    "txt",
    "wasm",
    "woff",
    "xhtml",
    "xml",
    "yaml",
    "yml",
};

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, Write},
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use super::{WalkOptions, build_walk, tmp_output_path, write_atomic};

    #[test]
    fn walk_respects_ignore_files_by_default() -> Result<()> {
        let root = test_dir("respect-ignore-default");
        fs::write(root.join(".ignore"), "ignored/\n")?;
        fs::create_dir(root.join("ignored"))?;
        fs::write(root.join("ignored/file.txt"), "data")?;
        fs::write(root.join("visible.txt"), "data")?;

        let entries = walk_paths(&root, &WalkOptions::default())?;
        assert!(entries.contains(&String::from("visible.txt")));
        assert!(!entries.iter().any(|path| path.starts_with("ignored/")));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn walk_respects_gitignore_in_git_repositories() -> Result<()> {
        let root = test_dir("respect-gitignore");
        fs::create_dir(root.join(".git"))?;
        fs::write(root.join(".gitignore"), "ignored/\n")?;
        fs::create_dir(root.join("ignored"))?;
        fs::write(root.join("ignored/file.txt"), "data")?;
        fs::write(root.join("visible.txt"), "data")?;

        let entries = walk_paths(&root, &WalkOptions::default())?;
        assert!(entries.contains(&String::from("visible.txt")));
        assert!(!entries.iter().any(|path| path.starts_with("ignored/")));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn walk_can_disable_ignore_files_and_apply_excludes() -> Result<()> {
        let root = test_dir("ignore-overrides");
        fs::write(root.join(".ignore"), "ignored/\n")?;
        fs::create_dir(root.join("ignored"))?;
        fs::write(root.join("ignored/keep.txt"), "data")?;
        fs::write(root.join("skip.txt"), "data")?;

        let options = WalkOptions {
            respect_ignore: false,
            exclude: vec![String::from("skip.txt")],
        };

        let entries = walk_paths(&root, &options)?;
        assert!(entries.contains(&String::from("ignored/keep.txt")));
        assert!(!entries.contains(&String::from("skip.txt")));

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn write_atomic_preserves_existing_output_on_failure() -> Result<()> {
        let root = test_dir("atomic-write-failure");
        let dst_path = root.join("asset.js.gz");
        let tmp_path = tmp_output_path(&dst_path);
        fs::write(&dst_path, b"existing artifact")?;

        let err = write_atomic(&dst_path, |dst| {
            dst.write_all(b"partial replacement")?;
            Err(io::Error::other("boom"))
        })
        .expect_err("write should fail");

        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(fs::read(&dst_path)?, b"existing artifact");
        assert!(!tmp_path.exists());

        fs::remove_dir_all(root)?;
        Ok(())
    }

    fn walk_paths(root: &Path, options: &WalkOptions) -> Result<Vec<String>> {
        let mut paths = build_walk(root, options)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                entry
                    .path()
                    .strip_prefix(root)
                    .ok()
                    .and_then(|path| (!path.as_os_str().is_empty()).then_some(path))
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
            })
            .collect::<Vec<_>>();
        paths.sort();
        Ok(paths)
    }

    fn test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("precompress-{name}-{unique}"));
        fs::create_dir_all(&root).expect("unable to create temp directory");
        root
    }
}
