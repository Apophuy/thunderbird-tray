# Architecture

`thunderbird-tray` is a new GPL-3.0-only application, not a Birdtray fork. It
has two cooperating processes:

- a Thunderbird 156+ Manifest V3 extension, authoritative for accounts,
  folders, and unread state;
- a Rust process, authoritative for lifecycle, configuration, tray and Linux
  desktop integration.

They communicate locally through WebExtension Native Messaging. The protocol,
Native Messaging framing, application state, tray integration, and
desktop-specific window control are separate boundaries. Native-host stdout is
reserved for framed protocol data; diagnostics must use stderr or structured
logging. No component reads Thunderbird profile databases or connects to mail
servers directly.

The versioned wire contract and framing rules are documented in
[`protocol.md`](protocol.md).

## Workspace boundaries

The initial workspace keeps the boundaries required by the Native Messaging
vertical slice:

- `thunderbird-tray`: executable and future native-host entry point;
- `thunderbird-tray-protocol`: typed, versioned JSON messages;
- `thunderbird-tray-native-messaging`: length framing and stream transport.

The application crate now contains transport-independent `core`, `config`,
`cli`, `doctor`, and `i18n` modules. They remain free of Thunderbird, D-Bus,
KDE, and Wayland APIs. A separate crate is introduced only when a boundary has
independent consumers or dependencies; Stage 3 does not split small modules
into speculative crates.

## Toolchain decision

The workspace uses Rust edition 2024 and declares Rust 1.85 as its minimum
supported version. Rust 1.85 is the release that stabilized edition 2024, and
the Stage 0 workspace has no third-party Rust dependencies imposing a newer
minimum. CI runs the declared toolchain so the claim remains tested.

The extension requires Node.js 22.13 or newer and uses locked npm dependencies.
It emits readable ES modules rather than bundled or minified output.

Stage 3 adds `toml` 1.1.6 for configuration. Stage 4 adds `ksni` 0.3.6 behind a
local adapter for StatusNotifierItem and DBusMenu. Stage 5 pins `zbus` 5.13.2,
the newest compatible release for the workspace's Rust 1.85 MSRV, and uses
`rustix` 1.1 for the service child's process-session boundary. The small fixed
CLI surface is parsed in the application crate instead of adding a general CLI
framework; its parser also distinguishes Mozilla's two Native Messaging launch
arguments from user commands.

## Identifiers

[`../identifiers.json`](../identifiers.json) is the canonical registry for the
product, application, native-host, and extension identifiers. The repository
owner is `Apophuy`; reverse-DNS identifiers normalize that component to
lowercase `apophuy`. The permanent extension ID is the generated UUID recorded
in that registry. Build and installation scripts consume the registry instead
of duplicating identifiers in source files.

## Localization and tray UX

English and Russian are the supported user-interface languages. English is the
source language and fallback. The `auto` language mode resolves Russian for
Russian system locales and English otherwise; explicit `en` and `ru` overrides
are persisted through the configuration boundary. Protocol values and
structured logs stay language-neutral.

Configuration discovery follows XDG rules, defaults safely when the implicit
file is absent, and treats an invalid or missing explicit file as fatal. Unknown
keys are rejected. See [`configuration.md`](configuration.md) for the schema,
CLI, and privacy-preserving `doctor` output.

The StatusNotifierItem icon, tooltip, and native DBusMenu are the MVP GUI.
Plasma owns the menu chrome, so the application focuses on a coherent icon
family, unambiguous normal/unread/disconnected states, concise action ordering,
keyboard-friendly native items, and a visible language selector. A standalone
settings window requires a later ADR and is not part of the current MVP.

The StatusNotifierItem adapter runs on its own service thread. A separate
Native Messaging reader publishes typed state changes, while one dedicated
writer owns stdout so framed protocol output cannot interleave. Tray callbacks
only enqueue typed actions. The pure presentation model and adapter menu tests
run without D-Bus, KDE, Thunderbird, or Wayland. See
[`adr/0001-status-notifier-item.md`](adr/0001-status-notifier-item.md) and
[`tray-ux.md`](tray-ux.md).

## Lifecycle and single instance

The application process owns the well-known session-bus name
`io.github.apophuy.thunderbird-tray`. A Thunderbird-launched native-host process
passes its stdin and stdout file descriptors to that owner and waits on a
separate completion descriptor. This keeps Thunderbird's host process contract
intact while one persistent process owns the tray. Only one Native Messaging
session is active at a time.

After the Native Messaging stream closes, the persistent process clears unread
data, presents the disconnected tray state, and waits for a subsequent
Thunderbird launch. The extension repeats the handshake and sends a complete
Inbox snapshot after reconnecting. StatusNotifierWatcher loss is also
recoverable: the tray service stays alive for automatic registration when the
watcher returns. See
[`adr/0002-single-instance-lifecycle.md`](adr/0002-single-instance-lifecycle.md).

## Window control and process launch

Window integration is capability-based: activate, hide, and show support are
reported independently. Backend selection records environment hints, runtime
availability, and its reason, while unsupported operations remain typed errors.
The initial `none` backend keeps the tray and unread monitoring operational on
desktops without global window control.

Open Thunderbird attempts activation only when the selected backend advertises
it. Otherwise it launches the configured executable and literal argument array
through `std::process::Command`, never a shell. Child standard input/output
cannot interfere with Native Messaging, and the persistent process reaps child
exit status. See
[`adr/0003-capability-based-window-control.md`](adr/0003-capability-based-window-control.md).
