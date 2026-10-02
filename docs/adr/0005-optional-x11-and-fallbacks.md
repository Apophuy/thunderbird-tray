# ADR 0005: Optional X11 backend and honest Wayland fallback

- Status: accepted
- Date: 2026-10-01

## Context

KDE Plasma Wayland has a runtime-probed KWin backend, but Linux desktops do not
share a generic protocol for controlling other clients' Wayland windows. X11
does provide the ICCCM and EWMH conventions, although the MVP's primary target
must not acquire an X11 or XWayland requirement.

Diagnostics also need to explain why a backend or tray is unavailable without
reading or exposing mail data.

## Decision

The X11 implementation is compiled only by the Cargo feature `x11`, which is
off by default. It uses `x11rb` 0.14's pure-Rust connection and core protocol;
libxcb FFI and X11 extension features are not enabled. Runtime selection
requires an X11 session, a reachable display, and the EWMH atoms used by the
implementation. State-changing requests are verified through window-manager
properties before success is returned.

GNOME, Sway, Hyprland, other wlroots compositors, and unidentified Wayland
sessions select the existing `none` backend. Open Thunderbird still uses the
structured process launcher, while tray and unread handling remain independent
of window control. No compositor-specific command, private protocol, portal
substitute, or XWayland fallback is added.

`doctor` performs read-only environment, D-Bus name-owner, backend, manifest,
and executable checks. The lifecycle service exposes only a boolean indicating
whether a Native Messaging stream is attached. Home paths are abbreviated and
untrusted environment strings are bounded and made single-line. Mail state and
Thunderbird arguments are excluded.

## Consequences

- Default builds and the KDE Wayland path do not depend on X11.
- X11 users can explicitly build a backend with detect, activate, minimize, and
  restore behavior on EWMH-compliant window managers.
- Unsupported Wayland desktops retain useful tray, unread, and process-launch
  behavior without false window-control claims.
- X11 window-manager variance remains a documented compatibility boundary and
  requires a real X11 live smoke test before a release claims a tested desktop.

## References

- <https://specifications.freedesktop.org/wm/latest-single/>
- <https://www.x.org/releases/current/doc/xorg-docs/icccm/icccm.html>
- <https://docs.rs/x11rb/0.14.0/x11rb/>
- <https://specifications.freedesktop.org/status-notifier-item/latest-single/>
