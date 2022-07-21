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
precompress 0.2.0
Precompress a directory of assets

USAGE:
    precompress [OPTIONS] <PATH>

ARGS:
    <PATH>    Directory to recursively compress files in

OPTIONS:
        --brotli <BROTLI>
            Enable brotli compression [possible values: true, false]

        --brotli-quality <BROTLI_QUALITY>
            Set brotli compression quality [default: 11]

        --deflate <DEFLATE>
            Enable deflate compression [possible values: true, false]

        --deflate-quality <DEFLATE_QUALITY>
            Set deflate compression quality [default: 9]

        --gzip <GZIP>
            Enable gzip compression [possible values: true, false]

        --gzip-quality <GZIP_QUALITY>
            Set gzip compression quality [default: 9]

    -h, --help
            Print help information

    -t, --threads <THREADS>
            Number of threads to use; "0" uses the number of cpus [default: 0]

    -V, --version
            Print version information

        --zstd <ZSTD>
            Enable zstd compression [possible values: true, false]

        --zstd-quality <ZSTD_QUALITY>
            Set zstd compression quality [default: 21]
```

### Example

Precompress the files in the current directory using brotli, deflate, and gzip:

```
precompress --brotli --deflate --gzip .
```

## TODO

- allow custom include/exclude globs
- add minimum file size constraint

## License

`precompress` is released under the MIT license.
Please see the [LICENSE](./LICENSE) file for more details.
