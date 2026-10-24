#!/bin/sh
# Fetch the original editable template directly from GNOME Design.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
url='https://gitlab.gnome.org/Teams/Design/HIG-app-icons/-/raw/master/template.svg'
output="$script_dir/gnome-app-icon-template.svg"
temporary="$output.tmp"

if ! command -v curl >/dev/null 2>&1; then
  echo 'curl is required to download the official GNOME template.' >&2
  exit 1
fi

trap 'rm -f "$temporary"' EXIT HUP INT TERM
curl --fail --location --silent --show-error "$url" --output "$temporary"
if ! grep -q '<svg' "$temporary"; then
  echo 'Download did not contain an SVG; template not installed.' >&2
  exit 1
fi
mv "$temporary" "$output"
trap - EXIT HUP INT TERM
printf 'Downloaded official GNOME template to %s\n' "$output"
