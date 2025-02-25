# VSC Contract Verifier

Verifies VSC contracts by compiling the contract source code and comparing the output bytecode CID against the deployed contract code CID.

Contract code compilation is done within Docker containers.

## Compile

Install [Rust](https://doc.rust-lang.org/book/ch01-01-installation.html) tools if not already, clone this repo and run the following:

```sh
cargo b -r
```

## Configuration

Dump a sample config file to `config.toml`:

```sh
./vsc-contract-verifier --dump-config
```

## License

This project is dual licensed under the [MIT License](https://github.com/techcoderx/vsc-contract-verifier/blob/main/LICENSE-MIT) or [Apache License 2.0](https://github.com/techcoderx/vsc-contract-verifier/blob/main/LICENSE-APACHE).
