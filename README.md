# VSC Blocks Backend

Backend server for [VSC Blocks](https://vsc.techcoderx.com). This is an extension of [go-vsc-node](https://github.com/vsc-eco/go-vsc-node) which provides the following services:

- Contract verifier
- REST API
- _(future)_ Chatbots

## Compile

Install [Rust](https://doc.rust-lang.org/book/ch01-01-installation.html) tools if not already, clone this repo and run the following:

```sh
cargo b -r
```

## Configuration

Dump a sample config file to `config.toml`:

```sh
./vsc-blocks-backend --dump-config
```

## Building compiler docker image

### AssemblyScript

```sh
cd as_compiler
docker build -t as-compiler .
```

## License

This project is dual licensed under the [MIT License](https://github.com/techcoderx/vsc-blocks-backend/blob/main/LICENSE-MIT) or [Apache License 2.0](https://github.com/techcoderx/vsc-blocks-backend/blob/main/LICENSE-APACHE).
