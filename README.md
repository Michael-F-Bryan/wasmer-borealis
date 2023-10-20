# wasmer-borealis

[![Continuous Integration](https://github.com/Michael-F-Bryan/wasmer-borealis/actions/workflows/ci.yml/badge.svg)](https://github.com/Michael-F-Bryan/wasmer-borealis/actions/workflows/ci.yml)

([API Docs][api-docs])

Named after the radiant northern lights, Wasmer Borealis is a tool designed to
shine a light on your package regressions and performance.

By running a wasmer CLI command on all packages in the Wasmer registry, Borealis
uncovers and highlights potential regressions, enabling you to enhance the
stability and reliability of your code.

Let Borealis bring the brilliance of your code to the forefront.

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
