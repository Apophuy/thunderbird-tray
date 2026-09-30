# Development

## Prerequisites

- Rust 1.85 or newer with `rustfmt` and `clippy`;
- Node.js 22.13 or newer and npm;
- Thunderbird 156 or newer for later integration checks (not needed in Stage
  0).

## Repository checks

Run the Rust checks from the repository root:

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Install the exact extension dependencies and run its checks:

```sh
npm --prefix extension ci
npm --prefix extension run lint
npm --prefix extension test
npm --prefix extension run build
```

The extension build is written to the ignored `extension/dist/` directory. Its
background entry point is intentionally inert until the protocol stage.

## Thunderbird documentation baseline

Extension changes must be checked against the Thunderbird 156 Manifest V3 API
documentation, rather than Firefox documentation or legacy Thunderbird APIs:

- <https://webextension-api.thunderbird.net/en/mv3/>
- <https://developer.thunderbird.net/add-ons/whats-new/manifest-v3>

The Stage 0 manifest requests no permissions. Later stages must justify every
permission with an implemented Thunderbird API call.
