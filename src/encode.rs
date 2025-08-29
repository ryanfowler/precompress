use std::fs::File;
use std::io::{Read, Result, Write};

use brotli::{
    BrotliCompressCustomAlloc,
    enc::{BrotliEncoderParams, StandardAlloc},
};
use flate2::{
    Compression,
    write::{DeflateEncoder, GzEncoder},
};
use zstd::Encoder;

use crate::precompress::Algorithm;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Quality {
    pub(crate) brotli: i8,
    pub(crate) deflate: i8,
    pub(crate) gzip: i8,
    pub(crate) zstd: i8,
}

impl Default for Quality {
    fn default() -> Self {
        Quality {
            brotli: 10,
            deflate: 7,
            gzip: 7,
            zstd: 19,
        }
    }
}

impl Quality {
    pub(crate) fn set(&mut self, algorithm: Algorithm, quality: i8) -> bool {
        match algorithm {
            Algorithm::Brotli => {
                if (0..=11).contains(&quality) {
                    self.brotli = quality;
                    true
                } else {
                    false
                }
            }
            Algorithm::Deflate => {
                if (1..=9).contains(&quality) {
                    self.deflate = quality;
                    true
                } else {
                    false
                }
            }
            Algorithm::Gzip => {
                if (1..=9).contains(&quality) {
                    self.gzip = quality;
                    true
                } else {
                    false
                }
            }
            Algorithm::Zstd => {
                if (-7..=22).contains(&quality) {
                    self.zstd = quality;
                    true
                } else {
                    false
                }
            }
        }
    }
}

pub(crate) struct Context {
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,

    brotli_quality: i32,
    deflate_quality: u32,
    gzip_quality: u32,
    zstd_quality: i32,
}

impl Context {
    pub(crate) fn new(buf_size: usize, quality: Quality) -> Self {
        Context {
            read_buf: vec![0; buf_size],
            write_buf: vec![0; buf_size],
            brotli_quality: quality.brotli as i32,
            deflate_quality: quality.deflate as u32,
            gzip_quality: quality.gzip as u32,
            zstd_quality: quality.zstd as i32,
        }
    }

    pub(crate) fn write_brotli(&mut self, input: &mut File, output: &mut File) -> Result<()> {
        let params = BrotliEncoderParams {
            quality: self.brotli_quality,
            ..Default::default()
        };
        BrotliCompressCustomAlloc(
            input,
            output,
            &mut self.read_buf,
            &mut self.write_buf,
            &params,
            StandardAlloc::default(),
        )?;
        Ok(())
    }

    pub(crate) fn write_deflate(&mut self, input: &mut File, output: &mut File) -> Result<()> {
        let output = BufWriter::new(output, &mut self.write_buf);
        let mut enc = DeflateEncoder::new(output, Compression::new(self.deflate_quality));
        loop {
            let n = input.read(&mut self.read_buf)?;
            if n == 0 {
                enc.finish()?;
                return Ok(());
            }
            enc.write_all(&self.read_buf[0..n])?;
        }
    }

    pub(crate) fn write_gzip(&mut self, input: &mut File, output: &mut File) -> Result<()> {
        let output = BufWriter::new(output, &mut self.write_buf);
        let mut enc = GzEncoder::new(output, Compression::new(self.gzip_quality));
        loop {
            let n = input.read(&mut self.read_buf)?;
            if n == 0 {
                enc.finish()?;
                return Ok(());
            }
            enc.write_all(&self.read_buf[0..n])?;
        }
    }

    pub(crate) fn write_zstd(&mut self, input: &mut File, output: &mut File) -> Result<()> {
        let mut enc = Encoder::new(output, self.zstd_quality)?;
        // Cap the HTTP window at 8 MiB (2^23) for browser support.
        enc.window_log(23)?;
        enc.long_distance_matching(false)?;
        loop {
            let n = input.read(&mut self.read_buf)?;
            if n == 0 {
                enc.finish()?;
                return Ok(());
            }
            enc.write_all(&self.read_buf[0..n])?;
        }
    }
}

struct BufWriter<'a, W: Write> {
    buf: &'a mut [u8],
    n: usize,
    w: W,
}

impl<'a, W: Write> BufWriter<'a, W> {
    fn new(w: W, buf: &'a mut [u8]) -> Self {
        BufWriter { buf, n: 0, w }
    }
}

impl<W: Write> Write for BufWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut p = 0;
        while p < buf.len() {
            let space = self.buf.len() - self.n;
            let src = if space > buf.len() - p {
                &buf[p..]
            } else {
                &buf[p..p + space]
            };

            if !src.is_empty() {
                self.buf[self.n..self.n + src.len()].copy_from_slice(src);
                self.n += src.len();
                p += src.len();
            }

            if self.buf.len() == self.n {
                self.flush()?;
            }
        }
        Ok(p)
    }

    fn flush(&mut self) -> Result<()> {
        let orig = self.n;
        self.n = 0;
        self.w.write_all(&self.buf[0..orig])
    }
}

impl<W: Write> Drop for BufWriter<'_, W> {
    fn drop(&mut self) {
        _ = self.flush();
    }
}
