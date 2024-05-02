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
  -c, --compression <COMPRESSION>  Compression algorithms to use
  -e, --extensions <EXTENSIONS>    Extensions of files that should be compressed
  -m, --min-size <MIN_SIZE>        Set the minimum size of files to be compressed in bytes [default: 1024]
  -t, --threads <THREADS>          Number of threads to use; "0" uses the number of cpus [default: 0]
  -h, --help                       Print help
  -V, --version                    Print version
```

By default, all compression algorithms are enabled. To specify the specific
types of compression you want to enable, you can use the `-c` flag with the
values:

- `br` or `brotli`
- `de` or `deflate`
- `gz` or `gzip`
- `zstd`

The compression quality can be specified by adding the value after a colon:

```
precompress -c gzip:5 .
```

There are number of file extensions that are compressed by default. To sepcify
the specific extensions to compress, you can use the `-e` flag like so:

```
precompress -e css -e json -e html .
```

or

```
precompress -e css,json,html .
```

### Example

Precompress the html files in the current directory using brotli and gzip with
a quality of 5, and a minimum file size of 4096:

```
precompress -c br:5,gz:5 -e html -m 4096 .
```

or

```
precompress -c br:5 -c gz:5 -e html -m 4096 .
```

## License

`precompress` is released under the MIT license.
Please see the [LICENSE](./LICENSE) file for more details.
