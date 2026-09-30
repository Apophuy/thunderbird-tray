# Protocol and Native Messaging framing

Protocol version 1 is a small tagged JSON contract shared by the Thunderbird
extension and the Rust host. Every message has exactly three conceptual parts:

```json
{
  "protocol": 1,
  "type": "hello",
  "payload": {}
}
```

The initial message types are `hello`, `helloAck`, `fullState`, and
`requestFullState`. Their canonical examples live in
[`../tests/fixtures/protocol/v1`](../tests/fixtures/protocol/v1) and are consumed
by both Rust and TypeScript tests. JSON field and type names use `camelCase`.

Unknown message types are recoverable: the Rust decoder returns only the
protocol number and unknown type so a caller can warn and continue without
retaining an unknown payload. Unsupported protocol versions and malformed known
messages are explicit errors.

## Framing

Native Messaging framing is handled separately from JSON. Each UTF-8 JSON
payload is prefixed by its length as a four-byte little-endian unsigned integer.
The transport accepts a clean EOF only before any prefix byte, distinguishes a
truncated prefix from a truncated payload, and validates size before allocating.

The project uses a symmetric one-mebibyte maximum frame. This matches Mozilla's
limit for messages emitted by a native application and is deliberately stricter
than the browser-to-application maximum because unread-state messages should be
small and bounded. Host stdout is reserved exclusively for these framed bytes;
logs and diagnostics belong on stderr.
