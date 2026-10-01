# ADR 0001: StatusNotifierItem implementation

- Status: accepted
- Date: 2026-09-30

## Context

The MVP needs a toolkit-free StatusNotifierItem and DBusMenu on the session bus.
It must update icons, tooltips, menu labels, and checked language state at
runtime while keeping the application-facing model independent of D-Bus.

The first live target is Debian 13 with KDE Plasma 6.3.6 on Wayland. Its
`org.kde.StatusNotifierWatcher` is present and reports protocol version 0. The
freedesktop StatusNotifierItem specification is still published as draft 0.1,
so tested implementation behavior matters alongside the protocol text.

## Decision

Use `ksni` 0.3.6 with its blocking API behind a local tray adapter.

Select the `async-io` runtime explicitly for both `ksni` and direct `zbus`
usage, with their default features disabled. The application entry point and
all of its D-Bus adapters are synchronous. Enabling `zbus`'s `tokio` feature
would make its internal task spawning require an ambient Tokio runtime even
when the blocking API creates a runtime for individual calls.

`ksni` implements both `org.kde.StatusNotifierItem` and
`com.canonical.dbusmenu` over `zbus`, registers with the watcher, emits property
and menu update signals, supports runtime updates and radio groups, and handles
known menu activation differences between Plasma before and after 6.4. Version
0.3.6 was released in July 2026, declares Rust 1.80, and therefore preserves
the project's Rust 1.85 MSRV.

The application owns a pure presentation model and action enum. `ksni` callbacks
only enqueue actions and never block on application work. D-Bus types do not
enter core state or the Native Messaging protocol.

## Alternatives considered

- Direct `zbus`: flexible, but it would duplicate substantial SNI watcher,
  properties, signals, pixmap, and DBusMenu layout/event logic already covered
  by `ksni`. Current `zbus` 5.19 also requires Rust 1.87, while Cargo can select
  a Rust-1.85-compatible `zbus` release through `ksni`.
- `system-tray`: actively maintained, but it is a client for implementations
  that render other applications' tray items rather than an application-side
  StatusNotifierItem service.
- Qt, GTK, AppIndicator, or a cross-platform tray toolkit: rejected because they
  add GUI toolkit dependencies and weaken the explicit SNI/DBusMenu boundary.

## Consequences

- Linux desktop protocol code stays small and replaceable.
- The blocking adapter runs its D-Bus service on a background thread, allowing
  the Native Messaging reader to remain synchronous for this stage.
- Lifecycle, window-control, diagnostics, and tray D-Bus calls share the
  `async-io` executor model and do not require an ambient Tokio runtime.
- Unit tests target the pure presentation and action boundary without a session
  bus. A separate live Plasma check remains required.
- `ksni` owns its generated per-process SNI bus name. The application's
  well-known single-instance name remains Stage 5 work.

## Evidence

- <https://specifications.freedesktop.org/status-notifier-item/latest-single/>
- <https://docs.rs/ksni/0.3.6/ksni/>
- <https://docs.rs/crate/ksni/0.3.6/source/Cargo.toml.orig>
- <https://github.com/iovxw/ksni>
