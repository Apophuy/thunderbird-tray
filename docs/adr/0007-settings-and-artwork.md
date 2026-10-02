# ADR 0007: Slint settings window and original application artwork

- Status: accepted
- Date: 2026-10-01

## Context

Stage 10 needs a normal, resizable settings window rather than a dense tray
submenu. It must run natively on the primary KDE Plasma Wayland target, support
English and Russian, offer light and dark appearance, preserve the Rust 1.85
MSRV, and avoid the explicitly excluded Qt, GTK, Electron, and Tauri stacks.

The application also needs an identity that is easy to distinguish from the
many blue tray icons. Mozilla's archived Thunderbird identity guide says the
Thunderbird logo must not be modified, overlaid, recolored, or shown without
its prescribed background. Mozilla's current trademark policy likewise does
not grant permission to create modified Mozilla marks. A state-changing tray
variant therefore cannot safely be based on that logo.

## Decision

Use pinned Slint 1.13.1 with its winit Wayland backend and software renderer for
the standalone settings window. It is part of the existing Rust executable;
there is no second service or network interface. The window has General, Tray,
and Thunderbird tabs, explicit Apply/Cancel behavior, and a `Palette` color
scheme selected from `system`, `light`, or `dark`. The saved TOML remains the
single configuration source. Apply notifies the running D-Bus service to reload
settings. If no service owns the application name yet, Apply starts a detached
service with the same executable and configuration path first. The settings
window has separate Cancel, Apply, and Done actions: Apply stays open and shows
the result, Done closes only after a successful apply, and Cancel discards
current unsaved edits. An About tab derives its version from Cargo package
metadata and shows the author and GPL license. The author's address is not
rendered; a mail-icon action passes it directly to `xdg-email` without a shell.
This current-session behavior is independent from XDG autostart, which controls
future sign-ins.

Slint 1.13.1 resets a `ComboBox` to its first row whenever its model reports a
change, including a model assigned while the window is first rendered. The
settings combo boxes therefore use compile-time, bilingual option lists rather
than models assigned from Rust. Ordinary labels still preview the chosen
language immediately, while language, theme, and backend indexes remain intact
through first render so Apply can persist them.

Slint's generated macro code needs to control its internal unsafe lint. The
workspace therefore changes `unsafe_code` from `forbid` to `deny`: handwritten
unsafe code remains rejected, while the audited dependency macro may apply its
internal allowance. Slint is GPL-compatible and its selected feature set does
not add Qt or GTK. Slint's semver range admits `fontdue` 0.9.4, whose source no
longer compiles with Rust 1.85, so the committed lockfile deliberately pins
`fontdue` 0.9.3 and verifies the full graph with the declared toolchain.

Use new, original pseudo-3D artwork: an amber bird holding a cream envelope on
a transparent background. The first artwork is the common application,
settings-window, desktop-launcher, connected-tray, and Thunderbird-extension
icon. The unread state keeps the bird but changes both envelope contours to red
and adds a bounded numeric badge; the disconnected state is muted and crossed
by a slash. The official Thunderbird logo is neither copied nor modified.

Install the application icon through standard hicolor sizes and a desktop
entry that opens `thunderbird-tray settings`. The same files are included in
the Debian package. The settings window may create or remove a
per-user XDG autostart entry; packages do not enable it automatically.

## Consequences

- Users receive a full settings window with system, light, and dark themes.
- The settings executable path is the same binary and cannot create a duplicate
  tray owner.
- The default dependency graph grows because of the GUI renderer and image
  decoder; release packaging derives the resulting shared-library dependencies.
- The original amber identity can change state without modifying a Mozilla
  trademark and remains legible against light or dark desktop themes.
- Autostart, Thunderbird launch, and start-hidden behavior remain explicit
  per-user choices and require no root access.
- Opening settings from the application menu can start the current tray session
  without waiting for Thunderbird to launch the Native Messaging host.

## References

- <https://www-archive.mozilla.org/foundation/identity-guidelines/thunderbird>
- <https://www.mozilla.org/foundation/trademarks/policy/>
- <https://docs.slint.dev/latest/docs/slint/guide/development/advanced/style/>
- <https://specifications.freedesktop.org/autostart-spec/latest/>
- <https://specifications.freedesktop.org/icon-theme-spec/latest/>
