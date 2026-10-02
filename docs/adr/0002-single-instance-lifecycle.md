# ADR 0002: Session service and Native Messaging stream handoff

- Status: accepted
- Date: 2026-09-30

## Context

Thunderbird owns the lifetime of a Native Messaging host process and supplies
the protocol stream on its standard input and output. The tray must outlive a
Thunderbird restart, while a second application start must not create another
StatusNotifierItem. A PID file cannot provide atomic ownership or recover
cleanly after crashes.

The extension must continue to use the standard Native Messaging API. The
application must not open a network listener or make the wire protocol share
stdout with diagnostics.

## Decision

The persistent process owns the well-known session-bus name
`io.github.apophuy.thunderbird-tray`. Ownership is requested without queueing
or replacement, so D-Bus atomically selects one process per user session. The lifecycle object
is exported at `/io/github/apophuy/thunderbird_tray/Lifecycle` using the
`io.github.apophuy.thunderbird_tray.Lifecycle` interface. These identifiers are
kept in `identifiers.json` and injected into the Rust build.

When Thunderbird launches the native-host executable, that short-lived
launcher connects to the primary process. If no owner exists, it starts an
internal `--service` child, which creates a new session before claiming the bus
name. The launcher passes its Native Messaging stdin and stdout file
descriptors to the primary process over D-Bus. The primary keeps framing and
protocol handling on dedicated reader and writer threads.

The primary returns one end of a private completion stream. The launcher waits
for EOF on that descriptor, keeping the process Thunderbird launched alive for
the complete Native Messaging session. The primary holds the other end until
the protocol reader finishes. Only one attached stream is accepted at a time;
another launcher receives an explicit error instead of creating a second tray
or interleaving protocol output.

After EOF or a framing/protocol failure, the primary clears the unread model,
shows the disconnected state, releases the active-session gate, and waits for
a new launcher. A new connection performs a new `hello`/`helloAck` exchange and
sends a complete state snapshot. Tray Refresh sends `requestFullState` only to
the currently attached session.

If the session bus is unavailable, a Thunderbird-launched process falls back
to a direct stdio host. This preserves the Native Messaging contract but cannot
provide the persistent tray or single-instance guarantee. An interactive
second application start reports that the application is already running.

The StatusNotifierItem service treats loss of `StatusNotifierWatcher` as
recoverable and remains alive so `ksni` can register it again when the watcher
returns. Quit shuts down the SNI service and drops the lifecycle bus name.

## Consequences

- Process uniqueness and crash cleanup follow session D-Bus ownership; no PID
  file or filesystem lock is needed.
- File-descriptor passing is Linux/Unix-specific, matching the project's Linux
  scope, and does not expose a socket path or network port.
- The primary process never borrows the launcher's stdio handles directly;
  ownership is explicit and closes naturally at session completion.
- Native Messaging framing and stdout purity remain unchanged.
- A launcher can fail clearly when an active session already exists.
- The hidden service mode is an implementation detail, not a user-facing
  daemon management interface.

## Alternatives considered

- Keeping one tray process per Thunderbird launch would lose the tray during a
  restart and allow duplicate StatusNotifierItems.
- PID files and advisory filesystem locks add stale-state recovery without
  carrying the Native Messaging stream.
- Forwarding every JSON message as a D-Bus method would duplicate framing and
  protocol flow-control responsibilities.
- A Unix-domain socket would require pathname ownership, permissions, and
  cleanup. Session D-Bus already supplies discovery, same-session routing, and
  file-descriptor passing.
