[![Build
Status](https://travis-ci.org/andrewjstone/rabble.svg?branch=master)](https://travis-ci.org/andrewjstone/rabble)

[API Documentation](https://docs.rs/rabble)

### Usage

Rabble only works with Rust versions > 1.12 and must use nightly builds for the compiler plugins.

Add the following to your `Cargo.toml`

```toml
[dependencies]
rabble = "0.1"
```

Add this to your crate root

```rust
extern crate rabble;
```
# Description
Rabble provides location independent actor communication over a fully connected mesh of nodes. More
information can be found in the [architecture
doc](https://github.com/andrewjstone/rabble/blob/master/doc/architecture.md) and [user
guide](https://github.com/andrewjstone/rabble/blob/master/doc/user_guide.md).
