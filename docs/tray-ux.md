# Tray UX specification

The tray is the MVP graphical interface. Plasma renders the menu itself;
`thunderbird-tray` supplies concise state, native actions, keyboard mnemonics,
and a restrained icon family.

## Visual states

| State | Icon | Tooltip | Stale data |
|---|---|---|---|
| Disconnected | muted envelope with a red disconnect slash | connection unavailable | never shown |
| Connected, zero | blue envelope | Inbox is up to date | not applicable |
| Connected, unread | blue envelope with a coral unread badge | localized Inbox unread count | not applicable |

All states remain visible by default. If `tray.hide_when_zero` is enabled, the
zero state becomes passive and Plasma may hide it. Unread state is active rather
than permanently requesting attention, avoiding distracting animation for a
persistent counter.

## Menu order

1. Open Thunderbird — present but disabled until the launch/backend stage wires
   the action.
2. Read-only Inbox status.
3. Refresh — asks the connected extension for a complete snapshot; disabled
   while disconnected.
4. Language submenu — radio choices Automatic, English, and Русский. A change
   updates all visible text immediately and persists to the config file.
5. Separator.
6. Quit — cleanly unregisters the tray service and exits.

When the Native Messaging stream closes, the persistent process remains visible
in the disconnected state so stale unread data is never mistaken for current
state. A later Thunderbird-launched host attaches to the same process, repeats
the handshake, and replaces the model with a fresh complete snapshot. Loss of
Plasma's StatusNotifierWatcher is recoverable and does not terminate the tray
service.

English and Russian labels use DBusMenu underscore mnemonics where practical.
The read-only status line explains the current state without requiring a
tooltip or icon interpretation.

## Accessibility and behavior

- Use standard DBusMenu items, radio state, disabled state, and icon names so
  Plasma supplies keyboard navigation and theme-consistent rendering.
- Never encode meaning through color alone: badge/slash geometry and text also
  distinguish states.
- Account names and other untrusted Thunderbird strings do not appear in the
  initial tray surface.
- Menu callbacks enqueue small typed actions and return immediately.
