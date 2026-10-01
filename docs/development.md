# Development

## Prerequisites

- Rust 1.85 or newer with `rustfmt` and `clippy`;
- Node.js 22.13 or newer and npm;
- Thunderbird 156 or newer for integration checks.

## Repository checks

Run the Rust checks from the repository root:

```sh
cargo build --workspace
cargo test --workspace --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The default build deliberately excludes X11. Compile and test the optional X11
backend separately with:

```sh
cargo build --workspace --features x11
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

## KDE Plasma Wayland tray check

After building and installing the development host as above, load the extension
in Thunderbird on a Plasma Wayland session. Verify the following without
restarting the host:

1. The tray item appears with the blue connected envelope after the handshake.
2. Changing the Inbox unread state changes the icon to or from its coral badge.
3. **Refresh Inbox status** requests a new complete snapshot and stays disabled
   while disconnected.
4. The tooltip and read-only menu status agree and never retain an old count
   after the extension disconnects.
5. **Language → Русский**, **English**, and **Automatic** update the open menu
   text and the next tooltip; the choice survives a restart in `config.toml`.
6. **Quit** removes the item from Plasma without leaving the SNI service
   registered.

For the Stage 5 lifecycle check:

1. Start the application once without Native Messaging arguments. A second
   interactive start must exit with an already-running diagnostic and must not
   add another tray item.
2. With the extension connected, close Thunderbird. The tray must immediately
   switch to disconnected and must not retain the previous unread count.
3. Start Thunderbird again. The same application process must accept the new
   Native Messaging stream, complete a new handshake, and show a fresh full
   Inbox state.
4. Restart Plasma shell or otherwise cycle `org.kde.StatusNotifierWatcher`.
   The application must stay alive and the tray item must register again when
   the watcher returns.

The persistent process uses the session D-Bus name
`io.github.apophuy.thunderbird-tray`; there are no PID files or root-owned
services. A Thunderbird-launched helper passes its standard streams to the
owner over the user session bus and stays alive until that Native Messaging
session completes.

## Thunderbird launch check

Set `thunderbird.command` to the local executable (currently
`/opt/thunderbird/thunderbird`) and optionally provide a TOML `arguments` array.
Select **Open Thunderbird** from the tray while Thunderbird is absent. The
configured executable must start, with each array value preserved as one
literal argument. On the `none` backend this launch fallback is expected; it
does not claim that an existing window was activated. Process activation on
unsupported desktops remains unavailable by design.

## Fallback and diagnostics check

Run `cargo run -p thunderbird-tray -- doctor` in the active graphical session.
Confirm that it reports the session type, desktop, session bus, application
service, extension connection, StatusNotifierWatcher, KWin, selected backend
and reason, X11 feature, native manifest, and Thunderbird executable. Paths
under the home directory must use `~`, and no account, unread, subject, sender,
or Thunderbird argument value may appear.

For a build intended for an X11 desktop, use `--features x11`. Verify on a real
X11 session—not XWayland in a Wayland session—that `doctor` selects `x11` and
that repeated activate, minimize, and restore actions are confirmed by the
window manager. If no real X11 session is available, record the live result as
untested. The detailed matrix and unsupported-Wayland behavior are in
[`fallbacks.md`](fallbacks.md).

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
