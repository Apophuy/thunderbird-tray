# Tray UX specification

The tray is the MVP graphical interface. Plasma renders the menu itself;
`thunderbird-tray` supplies concise state, native actions, keyboard mnemonics,
and a distinctive pseudo-3D icon family.

## Visual states

| State | Icon | Tooltip | Stale data |
|---|---|---|---|
| Disconnected | muted amber bird/envelope with a red disconnect slash | connection unavailable | never shown |
| Connected, zero | original amber bird holding a cream envelope | Inbox is up to date | not applicable |
| Connected, unread | amber bird with red envelope contours plus a white-on-navy numeric badge | localized Inbox unread count | not applicable |

All states remain visible by default. If `tray.hide_when_zero` is enabled, the
zero state becomes passive and Plasma may hide it. Unread state is active rather
than permanently requesting attention, avoiding distracting animation for a
persistent counter.

## Menu order

1. Open Thunderbird — tries supported window activation first and otherwise
   starts the configured executable with literal arguments.
2. Hide Thunderbird — minimizes it and removes it from KDE's task manager when
   the selected backend advertises that capability.
3. Read-only Inbox status.
4. Refresh — asks the connected extension for a complete snapshot; disabled
   while disconnected.
5. Settings — opens the full standalone settings window.
6. Language submenu — radio choices Automatic, English, and Русский. A change
   updates all visible text immediately and persists to the config file.
7. Separator.
8. Quit — cleanly unregisters the tray service and exits.

When the Native Messaging stream closes, the persistent process remains visible
in the disconnected state so stale unread data is never mistaken for current
state. A later Thunderbird-launched host attaches to the same process, repeats
the handshake, and replaces the model with a fresh complete snapshot. Loss of
Plasma's StatusNotifierWatcher is recoverable and does not terminate the tray
service.

Thunderbird remains authoritative for accounts and unread state. The tray
process never connects to mail servers directly, so it cannot check for new
mail while Thunderbird is closed; double activation starts Thunderbird and a
fresh extension handshake then replaces the disconnected state.

English and Russian labels use DBusMenu underscore mnemonics where practical.
The read-only status line explains the current state without requiring a
tooltip or icon interpretation.

Two primary activations within 500 ms open or activate Thunderbird. The SNI
protocol does not carry a click count, so the application recognizes the pair
of `Activate` calls; a single primary activation performs no action. The native
menu remains available from the context/right-click gesture.

On KDE Plasma Wayland, minimizing Thunderbird from the title bar also removes
it from the task manager while the tray service is running. The tray Open action
or double activation restores the task-manager entry before activating the
window.

## Accessibility and behavior

- Use standard DBusMenu items, radio state, disabled state, and icon names so
  Plasma supplies keyboard navigation and theme-consistent rendering.
- Never encode meaning through color alone: badge/slash geometry and text also
  distinguish states.
- The first original amber artwork is the common application, desktop, window,
  extension, and connected-state identity; only state overlays/variants change.
- Unread counts render as `1`–`99` and `99+` in a high-contrast white-on-navy
  badge positioned toward the icon's upper-right edge, while the tooltip and
  menu retain the exact aggregate count.
- Account names and other untrusted Thunderbird strings do not appear in the
  initial tray surface.
- Menu callbacks enqueue small typed actions and return immediately.

## Settings window

The settings command opens a normal resizable window with General, Tray, and
Thunderbird tabs. It exposes autostart, Thunderbird launch/start-hidden,
notifications, language, system/light/dark appearance, unread badge behavior,
command, and backend. Apply is enabled only when values differ from the last
successfully applied state; it persists, starts or notifies the service, and
keeps the window open with a result message. Done applies pending changes and
closes after success, or closes immediately when nothing changed. Cancel closes
without saving current edits. The window explicitly explains the close-button limitation
instead of implying that a post-close event can keep Thunderbird alive.
Opening the settings application ensures the current-session tray service is
running independently of Apply and without enabling autostart.
