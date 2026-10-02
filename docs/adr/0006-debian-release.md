# ADR 0006: Debian-only application package

- Status: accepted, amended for 0.2.0
- Date: 2026-10-01

## Context

The MVP needs an installable Linux artifact, a Thunderbird XPI, deterministic
release checks, and a path toward distribution. The primary user runs a Debian
system and prefers normal installation, upgrade, and removal through `apt` or
`dpkg`. The package also needs an absolute Native Messaging host path without
writing into one particular user's home directory during a privileged install.

The application already has a Native Messaging lifecycle. Adding an autostart
desktop file or systemd user unit would create a second owner for the same
process.

## Decision

The primary Linux artifact is a native `.deb`. Package-owned application files
live in `/opt/thunderbird-tray`, which is the FHS location for a self-contained
add-on package. Debian archive policy normally reserves `/opt` for local
administration, so this personal distribution package records a narrow Lintian
override for that deliberate layout. A package-owned link at
`/usr/bin/thunderbird-tray` exposes the command. The global Mozilla Native
Messaging manifest lives at
`/usr/lib/mozilla/native-messaging-hosts` and points to the absolute executable
path `/opt/thunderbird-tray/bin/thunderbird-tray`. It allows only the centralized
stable extension ID.

The package contains the optimized Rust binary, GPL-3.0-only licensing,
English and Russian readmes, example configuration, and the packaged XPI.
Users install or update the XPI through Thunderbird's Add-ons Manager; the
package does not modify a Thunderbird profile. It does not create a systemd
unit or enable autostart. Thunderbird starts the native helper through Native
Messaging, which connects to or starts the single-instance session service.
Stage 10 later adds a package-owned desktop launcher and hicolor icons, plus an
explicitly user-controlled per-user XDG autostart entry; see ADR 0007.

Starting with version 0.2.0, releases contain no portable `tar.xz` bundle and
the repository contains no portable installer or uninstaller. The supported
application package is the native `.deb`; the XPI remains separately available
for Thunderbird's Add-ons Manager. Development setup remains non-root, but it
is not a release distribution mechanism.

The XPI and `.deb` are deterministic. Release scripts fix
timestamps from `SOURCE_DATE_EPOCH`, sort archive members where applicable,
normalize package ownership, derive shared-library dependencies with
`dpkg-shlibdeps`, and produce SHA-256 checksums. The release workflow performs
two complete builds and compares every output byte. Content checks reject
agent/repository metadata, local checkout paths, missing licenses, unexpected
extension permissions, and invalid package/install behavior.

Cargo, the extension, XPI, and Debian package use one shared semantic version.
Every packaged update increments its patch component, and release handoffs
identify the complete version so an installed artifact is never confused with
a later rebuild.

## Consequences

- Debian users receive normal package-manager installation under `/opt`, with
  a machine-wide Native Messaging registration and clean package removal.
- The `.deb` build currently supports GNU/Linux `amd64` and `arm64` targets.
- Releases have one supported Linux installation path, avoiding a second
  installer and manifest lifecycle that must be maintained separately.
- Application and extension upgrades remain separate but share stable
  centralized identifiers.
- Starting the application before Thunderbird remains optional; no duplicate
  lifecycle manager is introduced.
- Removing the package deliberately leaves per-user configuration and the
  Thunderbird-installed extension untouched.

## References

- <https://refspecs.linuxfoundation.org/FHS_3.0/fhs/ch03s13.html>
- <https://www.debian.org/doc/debian-policy/ch-binary.html>
- <https://developer.thunderbird.net/add-ons/hello-world-add-on>
- <https://support.mozilla.org/en-US/kb/installing-addon-thunderbird>
- <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_manifests>
- <https://docs.github.com/actions/configuring-and-managing-workflows/persisting-workflow-data-using-artifacts>
