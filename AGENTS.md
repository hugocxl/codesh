# Agents

Guidance for AI coding agents working in this repository.

## Repository

- Codesh is a Rust workspace: the desktop app (`crates/codesh-app`, GPUI), the
  `codesh` CLI and terminal UI, `codesh-server`, and shared crates. Read
  [docs/product](docs/product/README.md) and [docs/tech](docs/tech/README.md)
  before larger changes.
- AI agents may commit to this repository when asked. Do not push, open pull
  requests, or comment on issues unless asked.

## Validation

Run before finishing:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

## Testing

- Only test real scenarios and risks (data loss, security, compatibility,
  concurrency, regressions). Don't test obvious code or defaults.

## Third-party dependencies

Ask before adding a dependency. Give its GitHub URL, star count, and whether it
is actively maintained, so the maintainer can approve it.

## Performance

- Do not ignore performance and memory efficiency. With GPUI it is easy to cause
  high CPU and memory usage: avoid work in render paths, layout feedback loops,
  and unnecessary re-renders.

## Code standards

- Prefer self-explanatory code over inline comments; doc comments (`///`) are
  fine.
- Early returns over nested conditionals.
- Fix root causes, not symptoms.
- Follow existing patterns.
