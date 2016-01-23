rust-cancellation [![Build Status](https://travis-ci.org/dgrunwald/rust-cancellation.svg?branch=master)](https://travis-ci.org/dgrunwald/rust-cancellation)
====================

Rust-Cancellation is a small [Rust](http://www.rust-lang.org/) crate that provides the `CancellationToken` type
that can be used to signal cancellation to other code in a composable manner.

* [Cargo package](https://crates.io/crates/cancellation)
* [Documentation](http://dgrunwald.github.io/rust-cancellation/doc/cancellation/)

---

Copyright (c) 2016 Daniel Grunwald.
[MIT license](http://opensource.org/licenses/MIT).

# Usage

To use `cancellation`, add this to your `Cargo.toml`:

```toml
[dependencies]
cancellation = { git = "https://github.com/dgrunwald/rust-cancellation.git" }
```

For more information, see the [documentation](http://dgrunwald.github.io/rust-cancellation/doc/cancellation/)
