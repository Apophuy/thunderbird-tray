#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(dirname -- "$script_dir")
release_dir=${1:-$repository_root/dist/release}
target=${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
version=$(cargo metadata --manifest-path "$repository_root/Cargo.toml" --no-deps --format-version 1 | node -e '
let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", chunk => input += chunk);
process.stdin.on("end", () => {
  const root = JSON.parse(input).packages.find(item => item.name === "thunderbird-tray");
  if (!root) throw new Error("thunderbird-tray package is missing");
  process.stdout.write(root.version);
});
')
xpi=$release_dir/thunderbird-tray-$version.xpi
checksums=$release_dir/SHA256SUMS
case "$target" in
    x86_64-unknown-linux-gnu) debian_architecture=amd64 ;;
    aarch64-unknown-linux-gnu) debian_architecture=arm64 ;;
    *)
        echo "unsupported Debian package target: $target" >&2
        exit 1
        ;;
esac
debian_version=$version
deb=$release_dir/thunderbird-tray_${debian_version}_${debian_architecture}.deb
native_host_name=$(node -e '
const fs = require("fs");
process.stdout.write(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).nativeHostName);
' "$repository_root/identifiers.json")
extension_id=$(node -e '
const fs = require("fs");
process.stdout.write(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).extensionId);
' "$repository_root/identifiers.json")

test -f "$xpi"
test -f "$deb"
test -f "$checksums"
test "$(wc -l < "$checksums")" -eq 2
if find "$release_dir" -maxdepth 1 -type f -name '*.tar.xz' -print | grep -q .; then
    echo "release directory contains a removed portable archive" >&2
    exit 1
fi
(cd "$release_dir" && sha256sum -c SHA256SUMS)
unzip -q -t "$xpi"
unzip -p "$xpi" LICENSE | cmp - "$repository_root/LICENSE"
unzip -p "$xpi" manifest.json | node -e '
let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", chunk => input += chunk);
process.stdin.on("end", () => {
  const manifest = JSON.parse(input);
  if (manifest.browser_specific_settings.gecko.id !== process.argv[1]) {
    throw new Error("packaged extension ID is incorrect");
  }
  if (manifest.permissions.join(",") !== "accountsRead,nativeMessaging") {
    throw new Error("packaged extension permissions changed");
  }
});
' "$extension_id"

temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT HUP INT TERM

test "$(dpkg-deb --field "$deb" Package)" = thunderbird-tray
test "$(dpkg-deb --field "$deb" Version)" = "$debian_version"
test "$(dpkg-deb --field "$deb" Architecture)" = "$debian_architecture"
test -n "$(dpkg-deb --field "$deb" Depends)"
deb_root=$temporary/deb-root
mkdir -p -- "$deb_root"
dpkg-deb --extract "$deb" "$deb_root"
deb_binary=$deb_root/opt/thunderbird-tray/bin/thunderbird-tray
deb_xpi=$deb_root/opt/thunderbird-tray/share/thunderbird-tray/thunderbird-tray.xpi
deb_manifest=$deb_root/usr/lib/mozilla/native-messaging-hosts/$native_host_name.json
deb_icon=$deb_root/usr/share/icons/hicolor/128x128/apps/io.github.apophuy.thunderbird-tray.png
test -x "$deb_binary"
"$deb_binary" --version | grep -F -x "thunderbird-tray $version" >/dev/null
test -f "$deb_xpi"
cmp "$deb_xpi" "$xpi"
test -f "$deb_root/opt/thunderbird-tray/LICENSE"
test -f "$deb_root/usr/share/doc/thunderbird-tray/copyright"
test -f "$deb_root/usr/share/man/man1/thunderbird-tray.1.gz"
test -f "$deb_root/usr/share/applications/io.github.apophuy.thunderbird-tray.desktop"
test -f "$deb_icon"
unzip -p "$xpi" icons/icon-128.png > "$temporary/extension-icon-128.png"
cmp "$deb_icon" "$temporary/extension-icon-128.png"
test -L "$deb_root/usr/bin/thunderbird-tray"
test "$(readlink "$deb_root/usr/bin/thunderbird-tray")" = \
    /opt/thunderbird-tray/bin/thunderbird-tray
node -e '
const fs = require("fs");
const [manifestPath, expectedName, expectedExtension] = process.argv.slice(1);
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
if (manifest.name !== expectedName) throw new Error("Debian native host name is incorrect");
if (manifest.path !== "/opt/thunderbird-tray/bin/thunderbird-tray") {
  throw new Error("Debian native host path is incorrect");
}
if (manifest.allowed_extensions[0] !== expectedExtension) {
  throw new Error("Debian extension ID is incorrect");
}
' "$deb_manifest" "$native_host_name" "$extension_id"
node - "$deb_binary" "$deb_manifest" "$extension_id" "$version" <<'NODE'
const { spawn } = require("child_process");
const [binary, manifestPath, extensionId, version] = process.argv.slice(2);
const hello = Buffer.from(JSON.stringify({
  protocol: 1,
  type: "hello",
  payload: {
    extensionVersion: version,
    thunderbirdVersion: "156.0.1",
  },
}));
const frame = Buffer.allocUnsafe(4 + hello.length);
frame.writeUInt32LE(hello.length, 0);
hello.copy(frame, 4);
const child = spawn(binary, [manifestPath, extensionId], {
  env: {
    ...process.env,
    XDG_CONFIG_HOME: "/tmp/thunderbird-tray-missing-release-config",
    DBUS_SESSION_BUS_ADDRESS: "invalid:",
  },
});
const stdout = [];
const stderr = [];
child.stdout.on("data", chunk => stdout.push(chunk));
child.stderr.on("data", chunk => stderr.push(chunk));
child.stdin.end(frame);
const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
child.on("error", error => {
  clearTimeout(timer);
  throw error;
});
child.on("close", code => {
  clearTimeout(timer);
  if (code !== 0) {
    throw new Error(`packaged native host failed: ${Buffer.concat(stderr).toString()}`);
  }
  const output = Buffer.concat(stdout);
  if (output.length < 4) throw new Error("packaged native host returned no frame");
  const length = output.readUInt32LE(0);
  if (output.length !== length + 4) {
    throw new Error("packaged native host returned invalid framing");
  }
  const response = JSON.parse(output.subarray(4).toString("utf8"));
  if (response.protocol !== 1 || response.type !== "helloAck") {
    throw new Error("packaged native host did not acknowledge the extension handshake");
  }
});
NODE
if find "$deb_root" -type f -exec grep -a -F -l "$repository_root" {} + | \
    grep -q .; then
    echo "Debian package contains the local repository path" >&2
    exit 1
fi
if command -v lintian >/dev/null 2>&1; then
    lintian --fail-on error "$deb"
fi

echo "Release artifacts passed Debian, XPI, Native Messaging, and privacy checks."
