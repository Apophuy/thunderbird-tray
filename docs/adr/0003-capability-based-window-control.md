# ADR 0003: Capability-based window control and process launch

- Status: accepted
- Date: 2026-09-30

## Context

Linux desktops do not provide one portable way to find, activate, hide, or show
an application window. In particular, a generic Wayland client cannot control
other clients' windows. Starting a process is separate from controlling an
existing window and remains useful even when every window operation is
unsupported.

The configuration must not be interpreted by a shell. A command or profile
argument can contain spaces and shell metacharacters and still needs to reach
Thunderbird as one literal argument.

## Decision

Window integration is represented by a `WindowControl` capability boundary.
Activate, hide, and show are independent capabilities. Operations return typed
outcomes and errors; an unsupported operation never reports success.

Backend selection combines the requested `auto`, `kde-wayland`, `x11`, or
`none` value with desktop environment hints and runtime backend availability.
It records the requested backend, selected backend, and reason. Stage 6 ships
the honest `none` implementation. KDE Wayland and optional X11 implementations
can be added behind the same boundary without changing tray or protocol code.

The Open Thunderbird action first uses activation when the selected backend
advertises that capability. An activated window completes the action. A
reported absence of a usable window, or a backend without activation support,
falls back to process launch.

The launcher passes the configured executable and argument array directly to
`std::process::Command`; it never concatenates a command string or invokes a
shell. Standard input and output are disconnected so a child cannot interfere
with Native Messaging framing. Standard error remains available for process
diagnostics. The long-lived application retains child handles and periodically
reaps completed launcher processes.

## Consequences

- The tray action works on unsupported Wayland desktops by launching
  Thunderbird, without pretending global activation succeeded.
- Backend-specific code cannot turn one supported operation into an assumption
  that all window operations are supported.
- Explicit but unavailable backends degrade to `none` with an observable
  reason, preserving unread monitoring and tray behavior.
- Arguments such as profile paths remain literal process arguments.
- Stage 7 can implement KDE activation without coupling KWin to core state,
  Native Messaging, or the tray presentation model.

## Alternatives considered

- Treating window control as a single supported/unsupported flag hides partial
  desktop capabilities and encourages fake success.
- Running a configured shell command makes quoting ambiguous and expands the
  command-injection surface.
- Disabling Open Thunderbird when activation is unsupported would discard the
  portable and useful process-launch fallback.
