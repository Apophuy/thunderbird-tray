#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(dirname -- "$script_dir")
host_name=$(node -e 'const fs = require("fs"); process.stdout.write(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).nativeHostName)' "$repository_root/identifiers.json")
manifest_path=${HOME:?}/.mozilla/native-messaging-hosts/$host_name.json

if [ -e "$manifest_path" ]; then
    rm -- "$manifest_path"
    echo "Removed native host manifest: $manifest_path"
else
    echo "Native host manifest is not installed: $manifest_path"
fi
