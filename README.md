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
> precompress --help
Precompress a directory of assets

Usage: precompress [OPTIONS] <PATH>

Arguments:
  <PATH>  Directory to recursively compress files in

Options:
      --brotli [<QUALITY>]   Enable brotli compression with optional quality [default: 11]
      --deflate [<QUALITY>]  Enable deflate compression with optional quality [default: 9]
      --gzip [<QUALITY>]     Enable gzip compression with optional quality [default: 9]
      --zstd [<QUALITY>]     Enable zstd compression with optional quality [default: 21]
      --min-size <MIN_SIZE>  Set the minimum size of files to be compressed in bytes [default: 1024]
  -t, --threads <THREADS>    Number of threads to use; "0" uses the number of cpus [default: 0]
  -h, --help                 Print help
  -V, --version              Print version
```

### Example

Precompress the files in the current directory using brotli and gzip:

```
precompress . --brotli --gzip
```

## TODO

- allow custom include/exclude globs

## License

`precompress` is released under the MIT license.
Please see the [LICENSE](./LICENSE) file for more details.
