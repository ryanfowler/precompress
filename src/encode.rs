use std::fs::File;
use std::io::{Read, Result, Write};

use brotli::{
    enc::{BrotliEncoderParams, StandardAlloc},
    BrotliCompressCustomAlloc,
};
use flate2::{
    write::{DeflateEncoder, GzEncoder},
    Compression,
};
use zstd::Encoder;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Quality {
    pub(crate) brotli: Option<u8>,
    pub(crate) deflate: Option<u8>,
    pub(crate) gzip: Option<u8>,
    pub(crate) zstd: Option<u8>,
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
            brotli_quality: quality.brotli.unwrap_or(11) as i32,
            deflate_quality: quality.deflate.unwrap_or(9) as u32,
            gzip_quality: quality.gzip.unwrap_or(9) as u32,
            zstd_quality: quality.zstd.unwrap_or(21) as i32,
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
