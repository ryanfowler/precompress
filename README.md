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
precompress 0.1.0
Precompress a directory of assets

USAGE:
    precompress [OPTIONS] <PATH>

ARGS:
    <PATH>    Directory to recursively compress files in

OPTIONS:
        --brotli <BROTLI>      Enable brotli compression [possible values: true, false]
        --deflate <DEFLATE>    Enable deflate compression [possible values: true, false]
        --gzip <GZIP>          Enable gzip compression [possible values: true, false]
    -h, --help                 Print help information
    -t, --threads <THREADS>    Number of threads to use (default: number of cpus) [default: 0]
    -V, --version              Print version information
        --zstd <ZSTD>          Enable zstd compression [possible values: true, false]
```

### Example

Precompress the files in the current directory using brotli, deflate, and gzip:

```
precompress --brotli --deflate --gzip .
```
