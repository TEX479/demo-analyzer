# Demo Analyzer (Rust)

This repo contains some tools for analysis of tf2 `.dem` files.

<!-- TODO: add some cool images -->

## Features
- a couple of decent algorithms
- a gui for analysing demo files and configuring the algorithms
- a tool for analysing demos while they are being recorded

## Getting started
More in-depth guides on installing & configuring the different tools are [here](docs/README.md). The following is a TL;DR:

> [!NOTE]
> This project is build using rust and requires `cargo` and friends, see
> [the rust website](https://www.rust-lang.org/) for how to get started.

Build the gui using
```bash
cargo build --bin=gui --features=gui --release
```
or the live analyzer using
```bash
cargo build --bin=live_analyzer --features=live_analyzer --release
```

which will place the corresponding binary at `target/release/<binary name>`.

You can provide the live analyzer with a parameter file by handing it the path as it's first argument:
```
live_analyzer params.json
```
or
```
live_analyzer /path/to/params.json
```