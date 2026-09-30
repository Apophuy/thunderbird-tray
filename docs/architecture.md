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

## Initial workspace

Stage 0 creates only the crates required by the first vertical slice:

- `thunderbird-tray`: executable and future native-host entry point;
- `thunderbird-tray-protocol`: typed, versioned JSON messages;
- `thunderbird-tray-native-messaging`: length framing and stream transport.

Tray, core-state, configuration, and window-backend crates will be introduced
only when their stages need them.

## Toolchain decision

The workspace uses Rust edition 2024 and declares Rust 1.85 as its minimum
supported version. Rust 1.85 is the release that stabilized edition 2024, and
the Stage 0 workspace has no third-party Rust dependencies imposing a newer
minimum. CI runs the declared toolchain so the claim remains tested.

The extension requires Node.js 22.13 or newer and uses locked npm dependencies.
It emits readable ES modules rather than bundled or minified output.

## Identifiers

[`../identifiers.json`](../identifiers.json) is the canonical registry for the
product, application, native-host, and extension identifiers. The repository
owner is `Apophuy`; reverse-DNS identifiers normalize that component to
lowercase `apophuy`. The permanent extension ID is the generated UUID recorded
in that registry. Build and installation scripts consume the registry instead
of duplicating identifiers in source files.

## Localization and tray UX

English and Russian are the supported user-interface languages. English is the
source language and fallback. A future `auto` language mode resolves Russian
for Russian system locales and English otherwise; explicit `en` and `ru`
overrides are persisted. Protocol values and structured logs stay
language-neutral.

The StatusNotifierItem icon, tooltip, and native DBusMenu are the MVP GUI.
Plasma owns the menu chrome, so the application focuses on a coherent icon
family, unambiguous normal/unread/disconnected states, concise action ordering,
keyboard-friendly native items, and a visible language selector. A standalone
settings window requires a later ADR and is not part of the current MVP.
