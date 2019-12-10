# kbdi

A helper tool for manipulating keyboards and locales on Windows. In general, this has no use case outside of keyboard installers.

## Building

This should always be built for the `i686-pc-windows-msvc` target. The `crt-static` flag must be applied to staticly link the C runtime.

```
cargo build --release --target i686-pc-windows-msvc --bin kbdi
cargo build --release --target i686-pc-windows-msvc --features legacy --bin kbdi-legacy
```

## License

`kbdi` is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.