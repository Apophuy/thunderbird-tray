#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(dirname -- "$script_dir")
release_dir=$repository_root/dist/release
target=${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
debian_maintainer=${DEB_MAINTAINER:-"Apophuy <apophuy@users.noreply.github.com>"}
version=$(cargo metadata --manifest-path "$repository_root/Cargo.toml" --no-deps --format-version 1 | node -e '
let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", chunk => input += chunk);
process.stdin.on("end", () => {
  const metadata = JSON.parse(input);
  const root = metadata.packages.find(item => item.name === "thunderbird-tray");
  if (!root) throw new Error("thunderbird-tray package is missing");
  process.stdout.write(root.version);
});
')
source_date_epoch=${SOURCE_DATE_EPOCH:-$(git -C "$repository_root" log -1 --format=%ct)}
zip_epoch=$source_date_epoch
if [ "$zip_epoch" -lt 315532800 ]; then
    zip_epoch=315532800
fi

node "$script_dir/check-release-metadata.mjs" "$version"
npm --prefix "$repository_root/extension" ci
npm --prefix "$repository_root/extension" run build
SOURCE_DATE_EPOCH=$source_date_epoch CARGO_INCREMENTAL=0 \
    cargo build --manifest-path "$repository_root/Cargo.toml" --locked --release \
    --target "$target" -p thunderbird-tray

rm -rf -- "$release_dir"
mkdir -p -- "$release_dir"
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT HUP INT TERM

xpi_name=thunderbird-tray-$version.xpi
xpi_stage=$temporary/xpi
mkdir -p -- "$xpi_stage"
cp -R -- "$repository_root/extension/dist/." "$xpi_stage/"
cp -- "$repository_root/LICENSE" "$xpi_stage/LICENSE"
find "$xpi_stage" -type f -exec touch -h -d "@$zip_epoch" {} +
xpi_path=$release_dir/$xpi_name
(
    cd "$xpi_stage"
    find . -type f -print | LC_ALL=C sort | zip -X -q "$xpi_path" -@
)

native_host_name=$(node -e '
const fs = require("fs");
process.stdout.write(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).nativeHostName);
' "$repository_root/identifiers.json")

case "$target" in
    x86_64-unknown-linux-gnu) debian_architecture=amd64 ;;
    aarch64-unknown-linux-gnu) debian_architecture=arm64 ;;
    *)
        echo "unsupported Debian package target: $target" >&2
        exit 1
        ;;
esac

debian_version=$version
deb_name=thunderbird-tray_${debian_version}_${debian_architecture}.deb
deb_root=$temporary/debian-package
deb_application=$deb_root/opt/thunderbird-tray
deb_manifest_directory=$deb_root/usr/lib/mozilla/native-messaging-hosts
deb_documentation=$deb_root/usr/share/doc/thunderbird-tray
deb_manpages=$deb_root/usr/share/man/man1
deb_lintian=$deb_root/usr/share/lintian/overrides
deb_applications=$deb_root/usr/share/applications
deb_icons=$deb_root/usr/share/icons/hicolor
mkdir -p -- \
    "$deb_root/DEBIAN" \
    "$deb_application/bin" \
    "$deb_application/share/thunderbird-tray" \
    "$deb_manifest_directory" \
    "$deb_documentation" \
    "$deb_manpages" \
    "$deb_lintian" \
    "$deb_applications" \
    "$deb_root/usr/bin"
install -m 0755 -- \
    "$repository_root/target/$target/release/thunderbird-tray" \
    "$deb_application/bin/thunderbird-tray"
install -m 0644 -- "$xpi_path" \
    "$deb_application/share/thunderbird-tray/thunderbird-tray.xpi"
install -m 0644 -- "$repository_root/packaging/config.example.toml" \
    "$deb_application/share/thunderbird-tray/config.example.toml"
install -m 0644 -- "$repository_root/LICENSE" "$deb_application/LICENSE"
install -m 0644 -- "$repository_root/README.md" "$deb_application/README.md"
install -m 0644 -- "$repository_root/README_RU.md" "$deb_application/README_RU.md"
install -m 0644 -- "$repository_root/assets/io.github.apophuy.thunderbird-tray.desktop" \
    "$deb_applications/io.github.apophuy.thunderbird-tray.desktop"
for size in 32 48 64 128 256; do
    icon_directory=$deb_icons/${size}x${size}/apps
    mkdir -p -- "$icon_directory"
    install -m 0644 -- \
        "$repository_root/assets/hicolor/${size}x${size}/apps/io.github.apophuy.thunderbird-tray.png" \
        "$icon_directory/io.github.apophuy.thunderbird-tray.png"
done
install -m 0644 -- "$repository_root/packaging/debian/copyright" \
    "$deb_documentation/copyright"
gzip -n -9 --stdout "$repository_root/packaging/debian/thunderbird-tray.1" \
    > "$deb_manpages/thunderbird-tray.1.gz"
install -m 0644 -- "$repository_root/packaging/debian/lintian-overrides" \
    "$deb_lintian/thunderbird-tray"
ln -s /opt/thunderbird-tray/bin/thunderbird-tray \
    "$deb_root/usr/bin/thunderbird-tray"

node - "$repository_root/identifiers.json" \
    "$deb_manifest_directory/$native_host_name.json" <<'NODE'
const fs = require("fs");
const identifiers = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const manifest = {
  name: identifiers.nativeHostName,
  description: "Native Messaging host for thunderbird-tray",
  path: "/opt/thunderbird-tray/bin/thunderbird-tray",
  type: "stdio",
  allowed_extensions: [identifiers.extensionId],
};
fs.writeFileSync(process.argv[3], `${JSON.stringify(manifest, null, 2)}\n`);
NODE

release_date=$(date --utc --date="@$source_date_epoch" --rfc-email)
changelog=$temporary/changelog.Debian
cat > "$changelog" <<EOF
thunderbird-tray ($debian_version) unstable; urgency=medium

  * Package upstream thunderbird-tray $version.

 -- $debian_maintainer  $release_date
EOF
gzip -n -9 --stdout "$changelog" > "$deb_documentation/changelog.gz"

mkdir -p -- "$temporary/debian"
cat > "$temporary/debian/control" <<EOF
Source: thunderbird-tray
Section: mail
Priority: optional
Maintainer: $debian_maintainer
Standards-Version: 4.7.4
Homepage: https://github.com/Apophuy/thunderbird-tray

Package: thunderbird-tray
Architecture: any
Description: Thunderbird tray companion
 Native Linux tray companion for Thunderbird 156 and newer.
EOF
shlib_dependencies=$(cd "$temporary" && dpkg-shlibdeps -O \
    -e"$deb_application/bin/thunderbird-tray" | sed -n 's/^shlibs:Depends=//p')
installed_size=$(du -sk "$deb_root/opt" "$deb_root/usr" | \
    awk '{ total += $1 } END { print total }')
cat > "$deb_root/DEBIAN/control" <<EOF
Package: thunderbird-tray
Version: $debian_version
Section: mail
Priority: optional
Architecture: $debian_architecture
Maintainer: $debian_maintainer
Homepage: https://github.com/Apophuy/thunderbird-tray
Depends: $shlib_dependencies
Installed-Size: $installed_size
Description: Thunderbird tray companion
 Native Linux tray companion for Thunderbird 156 and newer.
 Thunderbird provides unread state through a Manifest V3 extension and Native
 Messaging; the Rust application exposes a StatusNotifierItem on Linux.
EOF
find "$deb_root" -type d -exec chmod 0755 {} +
find "$deb_root" -type f -exec chmod 0644 {} +
chmod 0755 "$deb_application/bin/thunderbird-tray"
(cd "$deb_root" && find opt usr -type f -print | LC_ALL=C sort | xargs md5sum) \
    > "$deb_root/DEBIAN/md5sums"
find "$deb_root" -exec touch -h -d "@$source_date_epoch" {} +
SOURCE_DATE_EPOCH=$source_date_epoch dpkg-deb --build --root-owner-group \
    --uniform-compression --compression=xz --compression-level=9 \
    "$deb_root" "$release_dir/$deb_name"

(
    cd "$release_dir"
    sha256sum "$deb_name" "$xpi_name" > SHA256SUMS
)

echo "Release artifacts written to: $release_dir"
