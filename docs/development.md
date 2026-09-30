# Development

## Prerequisites

- Rust 1.85 or newer with `rustfmt` and `clippy`;
- Node.js 22.13 or newer and npm;
- Thunderbird 156 or newer for integration checks.

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

The extension build is written to the ignored `extension/dist/` directory.

## Thunderbird 156 Native Messaging check

Build both sides from the repository root:

```sh
cargo build -p thunderbird-tray
npm --prefix extension run build
```

Install the per-user native-host manifest. This writes only to
`~/.mozilla/native-messaging-hosts/` and does not require root:

```sh
./scripts/dev-install.sh
```

The script generates an ignored manifest with an absolute path to
`target/debug/thunderbird-tray`. To use another binary, pass its path as the
first argument. Remove the development registration with:

```sh
./scripts/dev-uninstall.sh
```

To load the extension temporarily:

1. Start Thunderbird 156 or newer. The currently verified local executable is
   `/opt/thunderbird/thunderbird`.
2. Open **Tools → Developer Tools → Debug Add-ons**.
3. Select **Load Temporary Add-on** and open
   `extension/dist/manifest.json`.
4. Select **Inspect** in the extension entry and open its Console. A successful
   connection reports `native messaging handshake completed`, followed by
   `sent Thunderbird Inbox unread state` with only `totalUnread` and
   `accountCount` diagnostic fields.

For the full handshake check, close any running Thunderbird process and start
it from a terminal so the native host's stderr remains visible:

```sh
/opt/thunderbird/thunderbird
```

After the extension loads, the terminal should show a completed Native
Messaging handshake followed by an Inbox unread-state message. The log contains
only the aggregate unread count and number of accounts; it does not contain mail
content. Native-host stdout is reserved for framed JSON and must not contain log
text. When Thunderbird was started from a desktop launcher, use the extension
Console described above instead of looking for host stderr in a terminal.

The generated manifest is named
`io.github.apophuy.thunderbird_tray.json`. Its `allowed_extensions` value comes
from the centralized permanent extension ID in `identifiers.json`.

## Thunderbird documentation baseline

Extension changes must be checked against the Thunderbird 156 Manifest V3 API
documentation, rather than Firefox documentation or legacy Thunderbird APIs:

- <https://webextension-api.thunderbird.net/en/mv3/>
- <https://developer.thunderbird.net/add-ons/whats-new/manifest-v3>

The extension currently requests only:

- `accountsRead`, for `accounts.list`, account events, Inbox discovery through
  `folders.query`, folder information, and folder events;
- `nativeMessaging`, for `runtime.connectNative`.

The implemented API contract was verified against these Thunderbird 156 pages:

- <https://webextension-api.thunderbird.net/en/mv3/accounts.html>
- <https://webextension-api.thunderbird.net/en/mv3/folders.html>
- <https://webextension-api.thunderbird.net/en/mv3/runtime.html>

The Linux manifest path and manifest fields follow Mozilla's Native Messaging
documentation:

- <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_manifests>
- <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging>
