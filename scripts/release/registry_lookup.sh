# Registry lookups shared by publish-registries.sh and verify-registries.sh;
# source after setting pypi_url, pypi_project, npm_registry, npm_package and
# version. Only an explicit not-found answer means "absent". A connection or
# TLS failure, any other HTTP status (401, 403, 429, 5xx, ...) or an
# unparseable body is fatal: an unverifiable registry state must never be
# mistaken for an empty one, since the caller would upload or verify on it.

# pypi_lookup <out-file>: prints "present" (release JSON written to out-file)
# or "absent" (HTTP 404); returns 1 on anything else.
pypi_lookup() {
  local out=$1 url="$pypi_url/pypi/$pypi_project/$version/json" code
  code=$(curl -sS -o "$out" -w '%{http_code}' "$url") \
    || { echo "PyPI lookup failed (curl exit $? for $url); cannot confirm whether $pypi_project $version is published" >&2; return 1; }
  case "$code" in
    200) python3 -I -c 'import json, sys; json.load(open(sys.argv[1]))' "$out" 2>/dev/null \
           || { echo "PyPI returned an unparseable body for $url" >&2; return 1; }
         echo present ;;
    404) rm -f "$out"; echo absent ;;
    *) echo "PyPI returned HTTP $code for $url; cannot confirm whether $pypi_project $version is published" >&2; return 1 ;;
  esac
}

# npm_lookup: prints the integrity npm reports for the package version, or
# "absent" when npm answers E404 (unknown package or version); returns 1 on
# anything else, with npm's own error on stderr.
npm_lookup() {
  local err out
  err=$(mktemp)
  if out=$(npm view "$npm_package@$version" dist.integrity --registry "$npm_registry" 2>"$err"); then
    rm -f "$err"
    [ -n "$out" ] || { echo "npm reports no dist.integrity for $npm_package@$version; cannot verify it" >&2; return 1; }
    printf '%s\n' "$out"
  elif grep -q '^npm error code E404$' "$err"; then
    rm -f "$err"; echo absent
  else
    cat "$err" >&2; rm -f "$err"
    echo "npm lookup for $npm_package@$version at $npm_registry failed; cannot confirm whether it is published" >&2
    return 1
  fi
}
