#!/usr/bin/env bash
# Gate: a Mach-O shared library is arm64, declares a macOS minimum no newer
# than the given floor, and links only system libraries and frameworks.
# Exit 1 otherwise. Runs on macOS (needs file, otool, codesign).
#
#   scripts/release/audit-macho.sh <shared-object> <max-macos>    e.g. ... 13.5
set -euo pipefail
file_=${1:?usage: audit-macho.sh <shared-object> <max-macos>}
max=${2:?usage: audit-macho.sh <shared-object> <max-macos>}
[ -f "$file_" ] || { echo "$file_: no such file" >&2; exit 1; }
for tool in file otool codesign; do command -v "$tool" >/dev/null || { echo "$tool is required" >&2; exit 1; }; done
kind=$(file -b "$file_") || { echo "$file_: file failed" >&2; exit 1; }
case "$kind" in
  *"Mach-O 64-bit"*"arm64"*) ;;
  *) echo "$file_: not a 64-bit arm64 Mach-O ('$kind')" >&2; exit 1 ;;
esac
case "$kind" in *"dynamically linked shared library"*|*"bundle"*) ;; *) echo "$file_: not a shared library or bundle ('$kind')" >&2; exit 1 ;; esac
loads=$(otool -l "$file_") || { echo "$file_: otool -l failed" >&2; exit 1; }
minos=$(printf '%s\n' "$loads" | awk '/LC_BUILD_VERSION/{b=1} b && /minos/{print $2; exit}')
[ -n "$minos" ] || minos=$(printf '%s\n' "$loads" | awk '/LC_VERSION_MIN_MACOSX/{b=1} b && /version/{print $2; exit}')
[ -n "$minos" ] || { echo "$file_: no LC_BUILD_VERSION or LC_VERSION_MIN_MACOSX load command" >&2; exit 1; }
highest=$(printf '%s\n%s\n' "$minos" "$max" | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)
[ "$highest" = "$max" ] || { echo "$file_: macOS minimum $minos exceeds $max" >&2; exit 1; }
# Dependencies are the names of the load commands that load or re-export a
# dylib. LC_ID_DYLIB is the file's own install name (an MH_DYLIB such as a
# Rust cdylib carries one, often @rpath/... or its build path) and is not a
# dependency; `otool -L` would list it alongside the real ones.
deps=$(printf '%s\n' "$loads" | awk '
  $1 == "cmd" { load = ($2 == "LC_LOAD_DYLIB" || $2 == "LC_LOAD_WEAK_DYLIB" || $2 == "LC_REEXPORT_DYLIB" || $2 == "LC_LOAD_UPWARD_DYLIB" || $2 == "LC_LAZY_LOAD_DYLIB") }
  load && $1 == "name" { print $2 }')
[ -n "$deps" ] || { echo "$file_: no dylib load commands (not linked against libSystem?)" >&2; exit 1; }
extra=$(printf '%s\n' "$deps" | grep -v -E '^(/usr/lib/libSystem\.B\.dylib|/System/Library/Frameworks/[A-Za-z]+\.framework/)' || true)
[ -z "$extra" ] || { echo "$file_: unexpected dynamic libraries: $(printf '%s' "$extra" | tr '\n' ' ')" >&2; exit 1; }
codesign -v "$file_" 2>/dev/null || { echo "$file_: code signature (ad hoc) does not verify" >&2; exit 1; }
printf '%s: arm64 Mach-O, macOS minimum %s, system libraries only, signature verifies\n' "$(basename "$file_")" "$minos"
