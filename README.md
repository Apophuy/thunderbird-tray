# thunderbird-tray

`thunderbird-tray` is a small Linux tray companion for Mozilla Thunderbird 156
and newer. A Thunderbird Manifest V3 extension will report unread state through
Native Messaging to a Rust process, which will provide toolkit-free Linux tray
integration with KDE Plasma Wayland as the first fully supported desktop.

This is a new implementation inspired by Birdtray's user-facing behavior. It is
not a Birdtray fork and does not reuse Birdtray source or architecture.

The project is currently in its repository-bootstrap stage; the extension and
binary do not yet exchange messages or display a tray item.

## Prerequisites

- Rust 1.85 or newer, including `rustfmt` and `clippy`;
- Node.js 22.13 or newer with npm;
- Thunderbird 156 or newer for later integration work.

## Build and test

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings

npm --prefix extension ci
npm --prefix extension run lint
npm --prefix extension test
npm --prefix extension run build
```

See [the development guide](docs/development.md) for details and
[the architecture overview](docs/architecture.md) for component boundaries and
the current identifier status.

## License

Copyright holders license this project under the GNU General Public License,
version 3 only (`GPL-3.0-only`). See [LICENSE](LICENSE).
