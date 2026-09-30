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

[`../identifiers.toml`](../identifiers.toml) is the canonical registry for the
product, application, native-host, and extension identifiers. Repository
ownership and the permanent extension ID are unresolved, so templates retain
`<owner>` and `<unresolved>`. The extension manifest deliberately omits a Gecko
ID until that product decision is made; this is sufficient for temporary
development loading but not packaged installation or Native Messaging
authorization.
