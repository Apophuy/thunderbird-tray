# ADR 0004: One-shot KWin scripts for Plasma Wayland window control

- Status: accepted
- Date: 2026-10-01

## Context

Wayland deliberately prevents an ordinary client from enumerating or
controlling another client's windows. KDE Plasma exposes the required window
objects to KWin JavaScript, including the stacking order, active window,
desktop-file name, resource class, minimization state, and minimizability.
Those APIs are documented for KWin 6. KWin also exposes a session D-Bus
scripting loader, but that loader and the per-script `run` object are less
stable than the documented JavaScript API.

The tray process needs synchronous, truthful results for detection,
activation, hiding, and showing. It must not require X11 or XWayland and must
not leave a permanently installed script behind.

## Decision

The `kde-wayland` backend supports Plasma/KWin 6.x. It is enabled only when all
of these runtime probes succeed:

1. the session identifies itself as KDE/Plasma on Wayland;
2. `org.kde.KWin` owns its session D-Bus name;
3. `/Scripting` accepts the `isScriptLoaded` method;
4. `XDG_RUNTIME_DIR` is an absolute usable path.

Each operation writes a unique one-shot JavaScript file below
`$XDG_RUNTIME_DIR/thunderbird-tray`, loads and runs it through KWin, waits for a
bounded result callback on the application's existing lifecycle D-Bus
interface, and then unloads the script and removes the file. The directory is
mode `0700` and scripts are created exclusively with mode `0600`.

The script searches KWin's top-to-bottom stacking order for Thunderbird by
`desktopFileName` first and by the known Thunderbird resource classes as a
compatibility fallback. It uses only KWin window properties for the operation:
`minimized`, `minimizable`, and `workspace.activeWindow`. Success is reported
only after the resulting KWin property reflects the requested change.

The documented KWin JavaScript API is the semantic integration boundary. The
D-Bus script loading/running surface is treated as compatibility-sensitive.
Failure of any probe or operation is logged and exposed as unavailable or a
typed operation failure; it never becomes fake success. Automatic selection
then falls back to the `none` backend, preserving the tray and unread state.

## Consequences

- Plasma Wayland window control needs neither X11 nor XWayland.
- No root installation, persistent KWin package, network listener, Qt, or GTK
  dependency is introduced.
- A KWin release that changes its compatibility-sensitive D-Bus loader will
  disable the backend safely until the adapter is updated.
- Operation latency is slightly higher than a permanent KWin script, but the
  lifecycle and privilege surface are smaller and cleanup is deterministic.
- Plasma/KWin 5 and non-KDE compositors are not claimed as supported by this
  backend. The implementation was verified locally with KWin 6.3.6.

## Alternatives considered

- X11/XWayland window matching would violate the native Wayland requirement
  and fail when Thunderbird runs as a native Wayland client.
- A permanently installed KWin package adds installation, upgrade, and stale
  state concerns for four short operations.
- KWin window rules are user configuration, do not provide the required
  request/result protocol, and are not suitable for transient activation.
- Command-line tools wrapping private KWin interfaces would add a runtime
  dependency without making the compatibility boundary more stable.

## References

- [KWin scripting API](https://develop.kde.org/docs/plasma/kwin/api/)
- [KWin scripting tutorial](https://develop.kde.org/docs/plasma/kwin/)
