# Configuration and CLI

`thunderbird-tray` reads TOML configuration from
`$XDG_CONFIG_HOME/thunderbird-tray/config.toml`. If `XDG_CONFIG_HOME` is unset
or relative, it falls back to `~/.config/thunderbird-tray/config.toml`. A missing
default file is valid and uses built-in defaults; a missing file explicitly
selected with `--config` is an error.

The parser rejects unknown keys and invalid enum values so spelling mistakes do
not silently change behavior. The complete initial schema is:

```toml
[general]
start_thunderbird = false
notifications = true
language = "auto"

[tray]
show_unread_count = true
hide_when_zero = false

[thunderbird]
command = "thunderbird"
arguments = []

[window]
backend = "auto"
```

Language values are `auto`, `en`, and `ru`. Automatic mode selects Russian for
a Russian process locale and otherwise falls back to English. The tray language
submenu changes visible text immediately and persists the selected mode through
this config boundary.

`thunderbird.command` is one executable path or program name and `arguments` is
an array of literal process arguments. Neither value is parsed by a shell. For
example:

```toml
[thunderbird]
command = "/opt/thunderbird/thunderbird"
arguments = ["--profile", "/home/user/Thunderbird Profile"]
```

Window backend values are `auto`, `kde-wayland`, `x11`, and `none`. Selection
combines the configured value with desktop hints and runtime backend
availability. An unavailable explicit backend degrades honestly to `none`; it
does not emulate successful window control. Stage 6 provides process launch and
the `none` backend. KDE Wayland window operations arrive in Stage 7, with X11
remaining an optional later fallback.

## Command line

```text
thunderbird-tray [OPTIONS] [doctor]

Options:
  --config <PATH>
  --window-backend <auto|kde-wayland|x11|none>
  --log-level <trace|debug|info|warn|error>
  -h, --help
  -V, --version
```

`doctor` currently reports the effective configuration path, whether it exists,
the selected window backend, language mode and resolved language, and the
configured Thunderbird command. It deliberately does not report mail, account,
or unread data. More desktop checks will be added in Stage 8.

When Thunderbird starts the binary as a Native Messaging host, Mozilla supplies
the native-manifest path and initiating extension ID as two positional process
arguments. They are recognized as a separate launch context and are not user
CLI commands or options. This behavior follows Mozilla's
[Native Messaging documentation](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging#connection-based_messaging).
