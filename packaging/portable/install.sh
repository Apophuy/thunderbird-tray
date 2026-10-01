#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

set -eu

bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=${THUNDERBIRD_TRAY_PREFIX:-${HOME:?}/.local}
data_home=${XDG_DATA_HOME:-${HOME:?}/.local/share}
binary_dir=$prefix/bin
data_dir=$data_home/thunderbird-tray
binary_path=$binary_dir/thunderbird-tray
application_dir=$data_home/applications
icon_root=$data_home/icons/hicolor

mkdir -p -- "$binary_dir" "$data_dir" "$application_dir"
install -m 0755 -- "$bundle_dir/bin/thunderbird-tray" "$binary_path"
install -m 0644 -- \
    "$bundle_dir/share/thunderbird-tray/thunderbird-tray.xpi" \
    "$data_dir/thunderbird-tray.xpi"
install -m 0644 -- \
    "$bundle_dir/share/thunderbird-tray/config.example.toml" \
    "$data_dir/config.example.toml"
install -m 0644 -- "$bundle_dir/LICENSE" "$data_dir/LICENSE"
install -m 0644 -- "$bundle_dir/README.md" "$data_dir/README.md"
install -m 0644 -- "$bundle_dir/README_RU.md" "$data_dir/README_RU.md"
install -m 0644 -- \
    "$bundle_dir/share/applications/io.github.apophuy.thunderbird-tray.desktop" \
    "$application_dir/io.github.apophuy.thunderbird-tray.desktop"
for size in 32 48 64 128 256; do
    icon_directory=$icon_root/${size}x${size}/apps
    mkdir -p -- "$icon_directory"
    install -m 0644 -- \
        "$bundle_dir/share/icons/hicolor/${size}x${size}/apps/io.github.apophuy.thunderbird-tray.png" \
        "$icon_directory/io.github.apophuy.thunderbird-tray.png"
done

"$binary_path" install-native-manifest

echo "Installed thunderbird-tray: $binary_path"
echo "Settings are available from the application menu or tray icon."
echo "Install the Thunderbird extension from: $data_dir/thunderbird-tray.xpi"
echo "Restart Thunderbird after installing or upgrading the extension."
