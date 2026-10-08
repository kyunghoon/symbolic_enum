# symbolic_enum

[![Crates.io](https://img.shields.io/crates/v/symbolic_enum.svg)](https://crates.io/crates/symbolic_enum)
[![Documentation](https://docs.rs/symbolic_enum/badge.svg)](https://docs.rs/symbolic_enum)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

A high-performance Rust derive macro for **compact, dual-mode Serde enums**.

`SymbolicEnum` combines bidirectional string conversions (`as_str`/`from_str`), compile-time FNV-1a numeric hashing, static iteration (`ALL_VARIANTS`/`iter`), and format-aware Serde serialization into a single, zero-dependency derive macro.

---

## Features

- **Dual-Mode Serde:** Serializes to human-readable variant strings in text formats (JSON, YAML, TOML) and compact **32-bit FNV-1a hashes** in binary formats (Bincode, Postcard).
- **Zero-Cost Conversions:** Inherent `as_str()`, `from_str()`, and standard `std::str::FromStr` implementations.
- **Built-in Iteration:** Provides `iter()` and `ALL_VARIANTS` slice without external dependencies like `strum`.
- **Compile-Time Safety:** Includes a const assertion guard that fails the build if two enum variants produce a 32-bit FNV-1a hash collision.

---

## Installation

Add `symbolic_enum` and `serde` to your `Cargo.toml`:

```toml
[dependencies]
symbolic_enum = "0.1"
serde = { version = "1.0", features = ["derive"] }