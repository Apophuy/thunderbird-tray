# thunderbird-tray

[Русская версия](README_RU.md)

`thunderbird-tray` is a small Linux tray companion for Mozilla Thunderbird 156
and newer. A Thunderbird Manifest V3 extension will report unread state through
Native Messaging to a Rust process, which will provide toolkit-free Linux tray
integration with KDE Plasma Wayland as the first fully supported desktop.

This is a new implementation inspired by Birdtray's user-facing behavior. It is
not a Birdtray fork and does not reuse Birdtray source or architecture.

The project is under active development. The current vertical slice connects a
Thunderbird 156+ extension to the Rust native host and reports aggregate unread
counts from Inbox folders. The tray interface is planned but is not implemented
yet.

## Prerequisites

- Rust 1.85 or newer, including `rustfmt` and `clippy`;
- Node.js 22.13 or newer with npm;
- Thunderbird 156 or newer for integration checks.

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
the current identifier status. Configuration and CLI options are documented in
[the configuration guide](docs/configuration.md). Development setup does not
require root access.

## Configuration

The default path is
`$XDG_CONFIG_HOME/thunderbird-tray/config.toml`, falling back to
`~/.config/thunderbird-tray/config.toml`. English and Russian are supported;
`language = "auto"` selects Russian for a Russian system locale and English
otherwise. See [`packaging/config.example.toml`](packaging/config.example.toml)
for all initial options.

Run `thunderbird-tray doctor` for a localized, privacy-preserving summary of the
effective configuration. Use `thunderbird-tray --help` for CLI options.

## Privacy

Thunderbird remains authoritative for accounts, folders, and unread state. The
extension sends only account identifiers, account display names, and aggregate
Inbox unread counts to the local Rust process through Native Messaging. It does
not send message subjects, bodies, sender addresses, credentials, or mail over
the network.

## Current limitations

- Only Inbox folders contribute to the unread total.
- The tray UI, language selector, and desktop window controls are planned for
  later MVP stages.
- Linux is the only supported platform; KDE Plasma Wayland is the first desktop
  target.

## License

Copyright holders license this project under the GNU General Public License,
version 3 only (`GPL-3.0-only`). See [LICENSE](LICENSE).
