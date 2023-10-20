# wasmer-borealis

[![Continuous Integration](https://github.com/Michael-F-Bryan/wasmer-borealis/actions/workflows/ci.yml/badge.svg)](https://github.com/Michael-F-Bryan/wasmer-borealis/actions/workflows/ci.yml)

([API Docs][api-docs])

Named after the radiant northern lights, Wasmer Borealis is a tool designed to
shine a light on your package regressions and performance.

By running a wasmer CLI command on all packages in the Wasmer registry, Borealis
uncovers and highlights potential regressions, enabling you to enhance the
stability and reliability of your code.

Let Borealis bring the brilliance of your code to the forefront.

## Getting Started

First, install the `wasmer-borealis` CLI. Probably the easiest way to do this
is via `cargo install`.

```console
$ cargo install --git https://github.com/Michael-F-Bryan/wasmer-borealis
```

Next, we need to create an experiment file to tell `wasmer-borealis` what to do.
Here is a `wapm2pirita.experiment.json` which will run `wapm2pirita convert` on
the latest version of each package either owned by `michael-f-bryan` or in
the `wasmer` namespace.

```json
{
  "$schema": "https://github.com/Michael-F-Bryan/wasmer-borealis/tree/main/experiment.schema.json",
  "package": "wasmer/wapm2pirita",
  "args": [
    "convert",
    "/files/${TARBALL_FILENAME}",
    "/out/${PKG_NAME}.webc"
  ],
  "wasmer": {
    "args": [
      "--mapdir=/files:${FIXTURES_DIR}",
      "--mapdir=/out:${OUT_DIR}"
    ]
  },
  "filters": {
    "users": [
      "michael-f-bryan"
    ],
    "namespaces": [
      "wasmer"
    ]
  }
}
```

Note that the `"$schema"` field isn't required. It's just an annotation to VS
Code which will let it know the format (technically, the [JSON Schema][schema])
that a `*.experiment.json` file takes.

Now, we can run the experiment:

```console
$ wasmer-borealis run ./example.experiment.json -o ./experiment
...
Experiment result... success: 0, failures: 53, bugs: 0. Finished in 201.794639333s
Experiment dir: ./experiment
```

Inside the `./experiment` directory, you will find the results of each experiment
run, plus a `report.html` summary for humans and a `results.json` summary that
can be used for further analysis.

```
$ tree ./experiment
experiment
├── michael-f-bryan
│   ├── cuboid-model
│   │   └── 0.1.4
│   │       ├── out
│   │       ├── stderr.txt
│   │       ├── stdout.txt
│   │       ├── test_case.json
│   │       └── working
│   │           ├── package.tar.gz
│   │           └── package.webc
...
├── report.html
└── results.json

222 directories, 269 files
```

### Environment Variable Interpolation

Several fields in the `*.experiment.json` file will expand environment variables.

The following environment variables are provided to each package:

| Variable           | Scope  | Example                                             | Description                                                             |
| ------------------ | ------ | --------------------------------------------------- | ----------------------------------------------------------------------- |
| `PKG_NAME`         | Common | `sha2`                                              | The package name                                                        |
| `PKG_VERSION`      | Common | `0.1.0`                                             | The package version                                                     |
| `WEBC_FILENAME`    | Common | `package.webc`                                      | The filename for the `*.webc` file, if available                        |
| `PKG_NAMESPACE`    | Common | `wasmer`                                            | The owner of the package                                                |
| `TARBALL_FILENAME` | Common | `package.tar.gz`                                    | The filename for the package's `*.tar.gz` file                          |
| `TARBALL_PATH`     | Host   | `./experiment/wasmer/sha2/0.1.0/out/package.tar.gz` | The absolute path for the `*.tar.gz` file on disk                       |
| `OUT_DIR`          | Host   | `./experiment/wasmer/sha2/0.1.0/out`                | A directory that any results should be saved to                         |
| `WEBC_PATH`        | Host   | `./experiment/wasmer/sha2/0.1.0/out/package.webc`   | The absolute path for the package's `*.webc` on the host                |
| `FIXTURES_DIR`     | Host   | `./experiment/wasmer/sha2/0.1.0/fixtures`           | The directory containing all package files downloaded from the registry |

The "Common" variables are available for both the package's arguments and the
`wasmer` CLI arguments, while "Host" variables will only be accessible to the
`wasmer` CLI.

Additionally, variables defined under `"env"` will be accessible in both scopes.

All variables from the host environment will be removed when constructing the
`wasmer run` command, with the exception of the following:

- `$PATH`
- `$WASMER_DIR`

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](./LICENSE-APACHE.md) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](./LICENSE-MIT.md) or
   <http://opensource.org/licenses/MIT>)

at your option.

It is recommended to always use [`cargo crev`][crev] to verify the
trustworthiness of each of your dependencies, including this one.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

The intent of this crate is to be free of soundness bugs. The developers will
do their best to avoid them, and welcome help in analysing and fixing them.

[api-docs]: https://michael-f-bryan.github.io/wasmer-borealis
[crev]: https://github.com/crev-dev/cargo-crev
[schema]: https://json-schema.org/
