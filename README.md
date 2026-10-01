# thunderbird-tray

[Русская версия](README_RU.md)

`thunderbird-tray` is a small Linux tray companion for Mozilla Thunderbird 156
and newer. A Thunderbird Manifest V3 extension will report unread state through
Native Messaging to a Rust process, which will provide toolkit-free Linux tray
integration with KDE Plasma Wayland as the first fully supported desktop.

This is a new implementation inspired by Birdtray's user-facing behavior. It is
not a Birdtray fork and does not reuse Birdtray source or architecture.

The project is under active development. The current implementation connects a
Thunderbird 156+ extension to the Rust native host and presents aggregate Inbox
unread state through a toolkit-free StatusNotifierItem and native DBusMenu.

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
[the configuration guide](docs/configuration.md). KDE Plasma integration and
its compatibility boundary are described in [the Wayland guide](docs/wayland.md).
Development setup does not require root access.

## Configuration

The default path is
`$XDG_CONFIG_HOME/thunderbird-tray/config.toml`, falling back to
`~/.config/thunderbird-tray/config.toml`. English and Russian are supported;
`language = "auto"` selects Russian for a Russian system locale and English
otherwise. See [`packaging/config.example.toml`](packaging/config.example.toml)
for all initial options.

Run `thunderbird-tray doctor` for a localized, privacy-preserving summary of the
effective configuration and desktop integration. It checks the session bus,
tray watcher, extension connection, selected backend, native manifest, and
Thunderbird executable without reporting mail data. Use `thunderbird-tray
--help` for CLI options.

The default build has no X11 dependency. On a real X11 desktop, enable the
optional EWMH backend with `cargo build -p thunderbird-tray --features x11`.
See the [fallback support matrix](docs/fallbacks.md).

## Tray interface

The tray distinguishes connected, unread, and disconnected states with its
icon and localized text. Its native menu shows the current Inbox status, can
request a fresh complete snapshot, and offers a persistent language selector
for Automatic, English, and Русский. On supported KDE Plasma Wayland sessions,
Open Thunderbird restores and activates the topmost existing Thunderbird
window; if no window exists, it launches the configured executable. Window
activation is capability-based and falls back to launch when the selected
backend cannot control an existing window.

The icons are embedded at multiple sizes, so the core tray UI does not depend
on an installed icon theme. Plasma renders the menu itself and supplies native
keyboard navigation and theming.

One persistent process owns the tray through a well-known user-session D-Bus
name. Closing Thunderbird changes the icon to disconnected without discarding
the tray process; starting Thunderbird again performs a new handshake and
replaces the display with a fresh Inbox snapshot. Duplicate starts do not
create duplicate tray items, and Plasma shell restarts are recoverable.

## Privacy

Thunderbird remains authoritative for accounts, folders, and unread state. The
extension sends only account identifiers, account display names, and aggregate
Inbox unread counts to the local Rust process through Native Messaging. It does
not send message subjects, bodies, sender addresses, credentials, or mail over
the network.

## Current limitations

- Only Inbox folders contribute to the unread total.
- Native detection, activation, hide, and show are supported on runtime-probed
  Plasma/KWin 6 Wayland sessions (tested with KWin 6.3.6). The KWin D-Bus
  script loader is compatibility-sensitive; failures degrade to process launch.
- X11 window controls are opt-in with the `x11` build feature and require an
  EWMH-compliant window manager; live X11 desktop verification is still pending.
- GNOME, Sway, Hyprland, and other Wayland compositors retain tray, unread, and
  process-launch features without claiming global window control. Tray display
  still requires a desktop StatusNotifierItem host.
- Linux is the only supported platform; KDE Plasma Wayland is the first desktop
  target.

## License

Copyright holders license this project under the GNU General Public License,
version 3 only (`GPL-3.0-only`). See [LICENSE](LICENSE).
