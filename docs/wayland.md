# KDE Plasma Wayland integration

## Support scope

`thunderbird-tray` supports native window detection, activation, hiding, and
showing on KDE Plasma Wayland with KWin 6.x. The Stage 7 implementation was
tested with KWin 6.3.6. It does not use X11 or XWayland and does not claim
compatibility with Plasma 5 or other Wayland compositors.

KWin's documented JavaScript API supplies window enumeration and state. The
session D-Bus methods used to load and run a script are
compatibility-sensitive, so the application probes them on every start instead
of assuming that the Plasma environment variables are sufficient.

## How it works

When `window.backend` is `auto` in a Plasma Wayland session, the application
checks that:

- the `org.kde.KWin` D-Bus service is present;
- its `/Scripting` object accepts the expected probe call;
- `XDG_RUNTIME_DIR` is an absolute usable directory.

Only then does it select `kde-wayland` and advertise all four operations. An
explicit `window.backend = "kde-wayland"` uses the same probes and degrades to
the unsupported `none` backend if they fail.

For an operation, the application creates a unique one-shot script under
`$XDG_RUNTIME_DIR/thunderbird-tray`. KWin finds the topmost Thunderbird window
using its desktop-file identity or resource class, applies the requested KWin
property change, and reports the observed result to the application's existing
session D-Bus service. The application unloads the script and removes the file
after success, error, or the two-second timeout.

The runtime directory is restricted to the current user (`0700`) and each
script is created without replacement at mode `0600`. No script is installed
globally, no root permission is needed, and no network socket is opened.

## Behavior and limitations

- **Open Thunderbird** activates and restores the topmost existing Thunderbird
  window. If no window exists, it launches the configured Thunderbird command.
- **Hide** minimizes the selected window only when KWin reports it as
  minimizable.
- **Show** restores the selected window and makes it active.
- Detection is based on `desktopFileName` values `thunderbird` or a path ending
  in `thunderbird.desktop`, with `thunderbird` and `thunderbird-default`
  resource classes as fallbacks.
- Multiple Thunderbird windows are handled by selecting the topmost matching
  window in KWin's stacking order. Per-profile selection is not part of the
  current protocol.
- KWin scripting can be disabled or changed by a distribution. In that case
  mail monitoring and the tray continue normally, while window control is
  reported unavailable and Open Thunderbird falls back to process launch.

## Troubleshooting

Run the application with debug logging and look for the selected backend and
probe error:

```sh
cargo run -p thunderbird-tray -- --log-level debug
```

Useful read-only checks on Plasma systems are:

```sh
qdbus6 org.kde.KWin /Scripting
qdbus6 org.kde.KWin /KWin supportInformation
```

The first command should list `isScriptLoaded`, `loadScript`, and
`unloadScript`. These commands are diagnostic only; `qdbus6` is not an
application runtime dependency. Also verify that the process receives
`XDG_SESSION_TYPE=wayland`, a KDE or Plasma `XDG_CURRENT_DESKTOP`, and an
absolute `XDG_RUNTIME_DIR`.

An opt-in live test is available while Thunderbird has a visible window and no
other `thunderbird-tray` process owns the application D-Bus name:

```sh
cargo test -p thunderbird-tray --test kde_wayland_live -- --ignored --nocapture
```

See [ADR 0004](adr/0004-kwin-one-shot-scripts.md) for the compatibility and
security decision.
