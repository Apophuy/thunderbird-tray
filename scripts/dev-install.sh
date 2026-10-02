#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(dirname -- "$script_dir")
binary_path=${1:-"$repository_root/target/debug/thunderbird-tray"}
manifest_dir=${HOME:?}/.mozilla/native-messaging-hosts
host_name=$(node -e 'const fs = require("fs"); process.stdout.write(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).nativeHostName)' "$repository_root/identifiers.json")
manifest_path=$manifest_dir/$host_name.json

if [ ! -x "$binary_path" ]; then
    echo "Native host is missing or not executable: $binary_path" >&2
    echo "Run: cargo build -p thunderbird-tray" >&2
    exit 1
fi

node "$script_dir/generate-native-manifest.mjs" "$binary_path" "$manifest_path"
echo "Installed native host manifest: $manifest_path"
