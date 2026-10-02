# Release and installation

## Build artifacts

From a clean checkout with the documented Rust and Node.js toolchains, plus
`dpkg-dev`, `zip`, and `xz-utils`:

```sh
./scripts/build-release.sh
./scripts/check-release.sh
```

Artifacts are written to and committed from `dist/release/`:

- `thunderbird-tray_<version>_<architecture>.deb` — primary Debian package;
- `thunderbird-tray-<version>.xpi` — Thunderbird 156+ MV3 extension;
- `SHA256SUMS` — hashes for all artifacts.

`./scripts/test-release-reproducibility.sh` performs two complete builds and
requires every output byte to match. `SOURCE_DATE_EPOCH` defaults to the current
Git commit timestamp and can be supplied explicitly by downstream builders.
Every packaged update increments the shared patch version used by Cargo, the
extension, XPI, and Debian package. `DEB_MAINTAINER` can override the local
package maintainer field.

The tag workflow requires a tag equal to `v<workspace-version>`, uploads the
checked artifacts to the workflow run, and creates a GitHub release with the
same files. It does not publish to addons.thunderbird.net or a Debian repository
automatically.

## Debian install

Verify the release directory and install the package:

```sh
sha256sum -c SHA256SUMS
sudo apt install ./thunderbird-tray_0.2.0_amd64.deb
```

The package installs:

- the application below `/opt/thunderbird-tray`;
- `/usr/bin/thunderbird-tray` as a package-owned command link;
- an application-menu launcher and hicolor icons using the common amber
  application/extension artwork;
- the global Mozilla Native Messaging manifest below
  `/usr/lib/mozilla/native-messaging-hosts`.

The manifest points to `/opt/thunderbird-tray/bin/thunderbird-tray`, so no
per-user setup step is required. The application itself still runs entirely in
the user's desktop session.

In Thunderbird 156 or newer, open **Add-ons and Themes → Extensions**, choose
**Install Add-on From File…**, and select
`/opt/thunderbird-tray/share/thunderbird-tray/thunderbird-tray.xpi`. Restart
Thunderbird if prompted. This supported Add-ons Manager flow avoids modifying a
Thunderbird profile behind Thunderbird's back.

## Debian upgrade and uninstall

Quit `thunderbird-tray` from its tray menu and install the new package with the
same `apt install ./…deb` command. Install the new XPI from Add-ons Manager and
restart Thunderbird. Stable application, native-host, and extension identifiers
preserve the connection across upgrades.

Remove the native package with:

```sh
sudo apt remove thunderbird-tray
```

Remove the extension separately in Thunderbird Add-ons Manager. Configuration
under `$XDG_CONFIG_HOME/thunderbird-tray` is deliberately retained because it
is user data rather than a package-owned file.

## Troubleshooting and logs

Run:

```sh
thunderbird-tray doctor
thunderbird-tray --log-level debug
```

`doctor` reports the manifest, executable, D-Bus services, extension
connection, and selected window backend without mail data. Runtime diagnostics
go to stderr. Native Messaging stdout is protocol-only. If the extension does
not connect, confirm that the manifest's `path` points to the installed
executable and the XPI has the stable extension ID. If no tray icon appears,
check the StatusNotifierWatcher line. KDE window-action limitations and probes
are documented in [`wayland.md`](wayland.md).

## Privacy and release contents

The release checks require GPL-3.0-only licensing in the XPI and Linux packages
and reject `PLAN.md`, `AGENTS.md`, `.agents`, `.codex`, `.git`, and the local
repository path. No mail content, credentials, Thunderbird profile, or user
configuration is included.

See [ADR 0006](adr/0006-debian-release.md) for the `/opt`,
Native Messaging, Debian-only release, and systemd decisions. The desktop entry,
application artwork, settings window, and opt-in autostart are covered by
[ADR 0007](adr/0007-settings-and-artwork.md).
