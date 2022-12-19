# precompress

Precompress a directory of assets

`precompress` will recursively compress all suitable assets in a given directory,
creating (or replacing) compressed versions of the original files using the
appropriate extension type (e.g. gzip: `index.html` -> `index.html.gz`).

## Installation

Using `cargo`:

```sh
cargo install precompress
```

## Usage

```
$ precompress -h
Precompress a directory of assets

Usage: precompress [OPTIONS] <PATH>

Arguments:
  <PATH>  Directory to recursively compress files in

Options:
      --brotli
          Enable brotli compression
      --deflate
          Enable deflate compression
      --gzip
          Enable gzip compression
      --zstd
          Enable zstd compression
      --brotli-quality <BROTLI_QUALITY>
          Set brotli compression quality [default: 11]
      --deflate-quality <DEFLATE_QUALITY>
          Set deflate compression quality [default: 9]
      --gzip-quality <GZIP_QUALITY>
          Set gzip compression quality [default: 9]
      --zstd-quality <ZSTD_QUALITY>
          Set zstd compression quality [default: 21]
  -t, --threads <THREADS>
          Number of threads to use; "0" uses the number of cpus [default: 0]
  -h, --help
          Print help information
  -V, --version
          Print version information
```

### Example

Precompress the files in the current directory using brotli, gzip, and zstd:

```
precompress . --brotli --gzip --zstd
```

## TODO

- allow custom include/exclude globs
- add minimum file size constraint

## License

`precompress` is released under the MIT license.
Please see the [LICENSE](./LICENSE) file for more details.
