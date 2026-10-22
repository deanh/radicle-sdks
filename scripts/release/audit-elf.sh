#!/usr/bin/env bash
# Gate: an ELF shared object is x86-64, needs no glibc symbol version above the
# given floor, and links only the base system libraries. Exit 1 otherwise.
#
#   scripts/release/audit-elf.sh <shared-object> <max-glibc>    e.g. ... 2.28
set -euo pipefail
file=${1:?usage: audit-elf.sh <shared-object> <max-glibc>}
max=${2:?usage: audit-elf.sh <shared-object> <max-glibc>}
[ -f "$file" ] || { echo "$file: no such file" >&2; exit 1; }
command -v objdump >/dev/null || { echo "objdump is required" >&2; exit 1; }
command -v ldd >/dev/null || { echo "ldd is required" >&2; exit 1; }
arch=$(objdump -f "$file" | sed -n 's/^architecture: \([^,]*\),.*/\1/p')
[ "$arch" = "i386:x86-64" ] || { echo "$file: architecture '$arch', expected i386:x86-64" >&2; exit 1; }
symbols=$(objdump -T "$file") || { echo "$file: objdump -T failed" >&2; exit 1; }
versions=$(printf '%s\n' "$symbols" | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -Vu || true)
[ -n "$versions" ] || { echo "$file: no GLIBC symbol versions found" >&2; exit 1; }
floor=$(printf '%s\n' "$versions" | tail -1)
highest=$(printf '%s\n%s\n' "$floor" "$max" | sort -V | tail -1)
[ "$highest" = "$max" ] || { echo "$file: glibc floor $floor exceeds $max" >&2; exit 1; }
libs=$(ldd "$file")
[[ "$libs" != *"not found"* ]] || { echo "$file: unresolved dynamic libraries" >&2; exit 1; }
allowed='(^|/)(linux-vdso|ld-linux-x86-64|libc|libm|libgcc_s|libpthread|libdl|librt)\.so'
extra=$(printf '%s\n' "$libs" | awk '{print $1}' | grep -v -E "$allowed" || true)
[ -z "$extra" ] || { echo "$file: unexpected dynamic libraries: $(printf '%s' "$extra" | tr '\n' ' ')" >&2; exit 1; }
printf '%s: x86-64, glibc floor %s, base libraries only\n' "$(basename "$file")" "$floor"
