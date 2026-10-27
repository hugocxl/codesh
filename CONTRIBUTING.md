# Contributing to Codesh

Codesh is developed and maintained by Hugo Corta. Bug reports, ideas and pull
requests are welcome on [GitHub](https://github.com/hugocxl/codesh).

## Getting started

Read [docs/product](docs/product/README.md) and [docs/tech](docs/tech/README.md)
first. They describe how Codesh works and how it is built.

### Prerequisites

- macOS 14+ for the desktop app, or Linux for the CLI and server
- [Rust](https://rustup.rs) (`rustup` installs the version pinned in `rust-toolchain.toml`)
- [Zig](https://ziglang.org) 0.15.2, used to build the Ghostty terminal library

### Setup

```bash
git clone https://github.com/hugocxl/codesh.git
cd codesh
```

On macOS, build the whole workspace with `cargo build --workspace`. On Linux,
build the CLI and server with `cargo build -p codesh-cli -p codesh-server`.

### Running

```bash
cargo run -p codesh-app     # desktop app (macOS)
cargo run -p codesh-cli     # codesh terminal UI
```

Build as in [Setup](#setup) first, so `codesh-server` sits next to the binary you
run. Development builds keep their data in a separate `Codesh Dev` profile
(`codesh-dev` on Linux).

## Development workflow

1. Create a branch from `main`.
2. Make your changes.
3. Run the [checks](#checks).
4. Open a pull request.

AI assistants and agents are welcome. They may write code and commit when you
ask them to; see [AGENTS.md](AGENTS.md) for the guidance they follow.

## Code standards

- **Self-explanatory code**: prefer clear structure over inline comments; doc
  comments (`///`) are fine
- **Early returns** over nested conditionals
- **Fix root causes**, not symptoms
- **Follow existing patterns**, but suggest refactors if they improve quality
- **Security first**: no command injection, XSS, or other vulnerabilities
- **Performance**: GPUI makes it easy to burn CPU and memory; keep render paths
  cheap
- **Dependencies**: ask before adding one, with its GitHub URL, star count and
  maintenance status

## Checks

All PRs must pass CI. Run its main checks locally:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

`cargo xcheck` and `cargo xtest` are aliases for the Clippy and test commands.

On Linux, CI runs two checks. `scripts/check-headless.py` covers the CLI, the
server, and the crates they depend on. It needs glibc 2.35; elsewhere, run Clippy
and tests with `-p` for each package that
`python3 scripts/check-headless.py --packages` lists, instead of `--workspace`.
`scripts/check-desktop-linux.py` covers the whole workspace, including the
[experimental desktop app](docs/tech/constraints.md), on Ubuntu 24.04.

## Pull requests

- Keep PRs focused on a single change
- Explain the "why" in the description
- Ensure all checks pass before requesting review
- Link any related issues

## Reporting issues

- Report bugs and ideas on [GitHub Issues](https://github.com/hugocxl/codesh/issues)
- Search existing issues before creating a new one
- Report security problems privately; see [SECURITY.md](SECURITY.md)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
