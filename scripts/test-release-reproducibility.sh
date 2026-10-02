#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-only

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(dirname -- "$script_dir")
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT HUP INT TERM

"$script_dir/build-release.sh"
"$script_dir/check-release.sh"
mkdir -p -- "$temporary/first"
cp -R -- "$repository_root/dist/release/." "$temporary/first/"
"$script_dir/build-release.sh"
"$script_dir/check-release.sh"

for first in "$temporary/first"/*
do
    name=$(basename -- "$first")
    cmp "$first" "$repository_root/dist/release/$name"
done

echo "Two consecutive release builds are byte-for-byte reproducible."
