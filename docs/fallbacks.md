# Fallback desktops and diagnostics

## Support matrix

Window control is separate from the tray and unread state. A missing window
backend never disables Native Messaging, the StatusNotifierItem service, or
the shell-free Thunderbird launch fallback.

| Environment | Window backend | Tray and unread behavior |
|---|---|---|
| KDE Plasma 6 Wayland | `kde-wayland` after the KWin runtime probe | Supported when a StatusNotifierWatcher is present |
| X11 with an EWMH-compliant window manager | Optional `x11` feature after an X11/EWMH probe | Supported when a StatusNotifierWatcher is present |
| GNOME Wayland | `none` | Continues if the desktop provides an SNI host; window actions fall back to launch |
| Sway, Hyprland, and other wlroots compositors | `none` | Continues if the desktop provides an SNI host; window actions fall back to launch |
| Other or unidentified sessions | `none` | Continues if session D-Bus and an SNI host are available |

There is no generic Wayland protocol for one application to control another
application's windows. The project therefore does not infer capabilities from
`WAYLAND_DISPLAY`, add compositor-specific commands, or route the KDE backend
through XWayland.

## Optional X11 backend

The default build has no X11 dependency. Build the optional backend with:

```sh
cargo build -p thunderbird-tray --features x11
```

The feature uses `x11rb`'s pure-Rust connection and core X11 protocol support;
it does not enable the libxcb FFI feature. At runtime the backend requires a
real X11 session and an EWMH window manager advertising
`_NET_CLIENT_LIST_STACKING`, `_NET_ACTIVE_WINDOW`, `_NET_WM_STATE`, and
`_NET_WM_STATE_HIDDEN`.

Thunderbird windows are matched narrowly by the `thunderbird` or
`thunderbird-default` `WM_CLASS`. Activation follows the EWMH
`_NET_ACTIVE_WINDOW` request. Minimize uses the ICCCM `WM_CHANGE_STATE`
request, and restore maps the window before activation. Each state-changing
operation waits for the window manager to expose the resulting active/hidden
state; a timeout is reported as failure rather than success.

The implementation follows the
[Extended Window Manager Hints specification](https://specifications.freedesktop.org/wm/latest-single/)
and the
[ICCCM window-state model](https://www.x.org/releases/current/doc/xorg-docs/icccm/icccm.html).

## `doctor`

Run:

```sh
thunderbird-tray doctor
```

The localized report includes:

- session type and desktop hints;
- session D-Bus, application service, KWin, and StatusNotifierWatcher state;
- whether the Thunderbird extension currently has a Native Messaging stream
  attached to the application service;
- requested and runtime-selected window backends plus the selection reason;
- whether X11 support was compiled in;
- the effective Native Messaging manifest path and whether it exists;
- whether the configured Thunderbird executable can be found.

Paths under the user's home directory are rendered with `~`. Environment
values are bounded and stripped of control characters. The report never reads
or prints account identifiers, account names, unread counts, subjects, sender
addresses, message content, Thunderbird arguments, or credentials.

## Verification record

The Stage 8 checks on 2026-10-01 covered:

| Environment | Result |
|---|---|
| KDE Plasma Wayland, KWin 6.3.6 | `doctor` selected `kde-wayland`, found the KDE watcher, KWin, and installed per-user manifest, and correctly reported that the default `thunderbird` command was absent from `PATH` |
| Simulated GNOME, Sway, and Hyprland Wayland hints | Backend selection tests retained `none`; no compositor capability was claimed |
| Default build | Built and tested without `x11rb` in the active dependency graph |
| Build with `--all-features` | X11 code compiled, linted, and passed unit tests; no real X11-only desktop was available for a live window-control smoke test |

The X11 live result is intentionally recorded as untested rather than inferred
from the XWayland server in the Plasma Wayland session.
