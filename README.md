# thunderbird-tray

[Русская версия](README_RU.md)

`thunderbird-tray` is a small Linux tray companion for Mozilla Thunderbird 156
and newer. A Thunderbird Manifest V3 extension reports unread state through
Native Messaging to a Rust process, which provides Linux tray integration and
a native settings window, with KDE Plasma Wayland as the first fully supported
desktop.

This is a new implementation inspired by Birdtray's user-facing behavior. It is
not a Birdtray fork and does not reuse Birdtray source or architecture.

The project is under active development. The current implementation connects a
Thunderbird 156+ extension to the Rust native host and presents aggregate Inbox
unread state through a StatusNotifierItem and native DBusMenu. A full settings
window controls startup, appearance, tray, and Thunderbird integration.

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

## Install

The primary release artifact is a Debian package. It installs the application
under `/opt/thunderbird-tray`, exposes `thunderbird-tray` through `/usr/bin`, and
registers the Native Messaging host system-wide. Install the packaged XPI from
Thunderbird's Add-ons Manager; the package never edits a Thunderbird profile.
See the [release and installation guide](docs/release.md) for install, upgrade,
and uninstall commands.

```sh
./scripts/build-release.sh
./scripts/check-release.sh
sudo apt install ./dist/release/thunderbird-tray_0.2.0_amd64.deb
```

Then install
`/opt/thunderbird-tray/share/thunderbird-tray/thunderbird-tray.xpi` through
**Add-ons and Themes → Extensions → Install Add-on From File…** in Thunderbird.

## Configuration

The default path is
`$XDG_CONFIG_HOME/thunderbird-tray/config.toml`, falling back to
`~/.config/thunderbird-tray/config.toml`. English and Russian are supported;
`language = "auto"` selects Russian for a Russian system locale and English
otherwise. See [`packaging/config.example.toml`](packaging/config.example.toml)
for all initial options.

Open **thunderbird-tray** from the application menu, choose **Settings…** from
the tray, or run `thunderbird-tray settings`. The resizable window offers
system, light, and dark themes. It can also install a per-user autostart entry,
launch Thunderbird with the service, and start Thunderbird hidden in the tray.
These startup options are disabled by default.
Opening the settings application also ensures that the tray service is running
for the current session; this does not enable autostart for future sign-ins.
Apply is enabled only for pending changes; it saves settings and starts the
current tray service when needed while keeping the window open. Done applies
pending changes and closes after success, or simply closes when nothing changed;
Cancel closes without saving current edits. Autostart separately controls
future sign-ins.

Run `thunderbird-tray doctor` for a localized, privacy-preserving summary of the
effective configuration and desktop integration. It checks the session bus,
tray watcher, extension connection, selected backend, native manifest, and
Thunderbird executable without reporting mail data. Use `thunderbird-tray
--help` for CLI options.

The default build has no X11 dependency. On a real X11 desktop, enable the
optional EWMH backend with `cargo build -p thunderbird-tray --features x11`.
See the [fallback support matrix](docs/fallbacks.md).

## Tray interface

The common application and extension icon is an original transparent pseudo-3D
amber bird holding an envelope. In unread state the outer and inner envelope
contours turn red and a badge displays `1`–`99` or `99+`; localized text keeps
the exact total. The native menu shows the current Inbox status, can request a
fresh complete snapshot, and offers a persistent language selector
for Automatic, English, and Русский. On supported KDE Plasma Wayland sessions,
Open Thunderbird restores and activates the topmost existing Thunderbird
window; if no window exists, it launches the configured executable. Window
activation is capability-based and falls back to launch when the selected
backend cannot control an existing window.
Double-activating the tray icon opens or activates Thunderbird; use the
context/right-click gesture for the native menu.

On supported KDE Plasma Wayland sessions, **Hide Thunderbird** minimizes the
window and removes it from the task manager without stopping mail monitoring.
The public Thunderbird/KWin interfaces cannot intercept the title-bar close
button before the window closes, so the explicit Hide action is used instead.
The normal minimize button also removes the window from the task manager while
`thunderbird-tray` is running; use the tray menu or double activation to restore
it.

The icons are embedded at multiple sizes, so the core tray UI does not depend
on an installed icon theme. Plasma renders the menu itself and supplies native
keyboard navigation and theming.

One persistent process owns the tray through a well-known user-session D-Bus
name. Closing Thunderbird changes the icon to disconnected without discarding
the tray process; starting Thunderbird again performs a new handshake and
replaces the display with a fresh Inbox snapshot. Duplicate starts do not
create duplicate tray items, and Plasma shell restarts are recoverable.
The tray process does not connect to mail servers itself, so no mail is checked
while Thunderbird is closed.

## Troubleshooting

Run `thunderbird-tray doctor` for the effective integration status and
`thunderbird-tray --log-level debug` for detailed stderr diagnostics. If the
extension does not connect, verify the Native Messaging manifest and installed
binary reported by `doctor`. If the icon is absent, check that the desktop has
a StatusNotifierItem host. See the [release guide](docs/release.md) and
[Wayland guide](docs/wayland.md) for detailed checks and known limitations.

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
