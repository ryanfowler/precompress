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
precompress 0.2.1
Precompress a directory of assets

USAGE:
    precompress [OPTIONS] <PATH>

ARGS:
    <PATH>    Directory to recursively compress files in

OPTIONS:
        --brotli
            Enable brotli compression

        --brotli-quality <BROTLI_QUALITY>
            Set brotli compression quality [default: 11]

        --deflate
            Enable deflate compression

        --deflate-quality <DEFLATE_QUALITY>
            Set deflate compression quality [default: 9]

        --gzip
            Enable gzip compression

        --gzip-quality <GZIP_QUALITY>
            Set gzip compression quality [default: 9]

    -h, --help
            Print help information

    -t, --threads <THREADS>
            Number of threads to use; "0" uses the number of cpus [default: 0]

    -V, --version
            Print version information

        --zstd
            Enable zstd compression

        --zstd-quality <ZSTD_QUALITY>
            Set zstd compression quality [default: 21]
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
