#!/usr/bin/env bash
# Publish one registered release to PyPI and npm: exactly the bytes the
# rad-artifact receipt records, nothing else, and nothing before every check
# passes. The receipt (release object on the tag, with MANIFEST.json) is the
# signed record of what was built; the registries are where consumers install
# from. Run on the Linux build host, where release/<version>/ lives.
#
#   scripts/release/publish-registries.sh <version> [--dry-run]
#
# Preflight, each fatal and all before any upload:
# - release/<version>/MANIFEST.json names this version and exactly the files
#   its targets call for, each present with the recorded digest and size;
# - the local v<version> is an annotated tag whose object equals the one on
#   the rad remote's push URL, and its commit equals the manifest's source;
# - the receipt is registered: exactly one release object is bound to the tag
#   and its live artifacts are exactly the manifest's files plus MANIFEST.json,
#   each with the content hash (CID) of the local file (register.sh first);
# - the registries hold nothing conflicting: a file already present under
#   this version must have the same digest (it is skipped), and PyPI must hold
#   no file this release did not build;
# - credentials exist for whatever is still to upload: UV_PUBLISH_TOKEN for
#   PyPI, a logged-in npm user for npm. UV_PUBLISH_TOKEN comes from the
#   environment or from `.env` at the repository root, which may hold only
#   that one NAME=value line (plus comments) and is used only when the
#   environment leaves the name empty. The file must be ignored by git and
#   readable by its owner only; it is parsed, not executed, and never printed.
# A registry lookup that fails for any reason other than an explicit
# not-found stops the run: an unverifiable registry state is not an empty one.
# --dry-run stops after printing the plan. Uploads: `uv publish` for the
# pending Python files (with --check-url as a second guard against
# duplicates), then `npm publish` of the exact tarball (npm may prompt for a
# one-time password). Interrupted runs are resumed by rerunning. Afterwards,
# verify from the registries on each platform: verify-registries.sh.
set -euo pipefail
version=${1:?usage: publish-registries.sh <version> [--dry-run]}
dry_run=${2:-}
case "$dry_run" in ""|--dry-run) ;; *) echo "usage: publish-registries.sh <version> [--dry-run]" >&2; exit 2 ;; esac
root=$(git rev-parse --show-toplevel)
cd "$root"
here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=registry_lookup.sh
. "$here/registry_lookup.sh"
dir="release/$version"
tag="v$version"
manifest="$dir/MANIFEST.json"
pypi_project=radicle-py
npm_package=radicle-sdk
pypi_url=${PYPI_URL:-https://pypi.org}
pypi_upload_url=${PYPI_UPLOAD_URL:-https://upload.pypi.org/legacy/}
npm_registry=${NPM_REGISTRY:-https://registry.npmjs.org}
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT

echo "== release directory"
python3 "$here/release_files.py" check "$dir" "$version" > "$work/names"
tarball="$npm_package-$version.tgz"
python_files=()
while read -r name; do
  case "$name" in *.whl|*.tar.gz) python_files+=("$name") ;; esac
done < "$work/names"
[ "${#python_files[@]}" -gt 0 ] || { echo "manifest lists no Python artifacts" >&2; exit 1; }

echo "== tag"
revision=$(git rev-parse --verify -q "refs/tags/$tag") || { echo "no local tag $tag" >&2; exit 1; }
[ "$(git cat-file -t "$revision")" = tag ] || { echo "$tag is not an annotated tag" >&2; exit 1; }
push_url=$(git remote get-url --push rad)
remote=$(git ls-remote "$push_url" "refs/tags/$tag" | awk '$2 == "refs/tags/'"$tag"'" {print $1}')
[ -n "$remote" ] || { echo "push the tag first: git push rad $tag (not found at $push_url)" >&2; exit 1; }
[ "$remote" = "$revision" ] || { echo "remote $tag is $remote, local is $revision; they must be the same tag object" >&2; exit 1; }
commit=$(git rev-parse "$revision^{commit}")
m_source=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["source"])' "$manifest")
[ "$commit" = "$m_source" ] || { echo "$tag points at $commit but the artifacts were built from $m_source" >&2; exit 1; }

echo "== receipt on rad-artifact"
rid=$(rad .)
rad-artifact node status >/dev/null 2>&1 || { echo "start the artifact node first: rad-artifact node start" >&2; exit 1; }
: > "$work/local-cids"
while read -r name; do printf '%s %s\n' "$(rad-artifact cid "$dir/$name")" "$name" >> "$work/local-cids"; done < <(cat "$work/names"; echo MANIFEST.json)
rad-artifact --repo "$rid" list --json > "$work/listing.json"
python3 - "$work/listing.json" "$work/local-cids" "$revision" <<'PY'
import json, sys
listing, local_cids, revision = sys.argv[1:4]
local = dict(line.split()[::-1] for line in open(local_cids).read().splitlines())  # name -> cid
releases = [r for r in json.load(open(listing))["releases"] if r.get("tag") == revision]
if len(releases) != 1:
    sys.exit(f"{len(releases)} release objects are bound to this tag; expected exactly one (register.sh first)")
live = {}
for a in releases[0]["artifacts"]:
    if not a["redactions"]:
        live.setdefault(a["name"], set()).add(a["cid"])
problems = [f"{n}: not registered" for n in local if n not in live]
problems += [f"{n}: live artifact this release did not build" for n in live if n not in local]
problems += [f"{n}: live content {', '.join(sorted(c))} differs from the local file {local[n]}" for n, c in live.items() if n in local and c != {local[n]}]
if problems:
    sys.exit("the receipt on this tag is not the local release; run register.sh first or publish a new version:\n  " + "\n  ".join(problems))
print(f"receipt {releases[0]['releaseId']} holds exactly the {len(local)} local files")
PY

echo "== registry state"
pypi_state=$(pypi_lookup "$work/pypi.json")
case "$pypi_state" in present) pypi_json="$work/pypi.json" ;; absent) pypi_json=- ;; esac
python3 "$here/registry_state.py" pypi-plan "$manifest" "$pypi_json" > "$work/pypi-plan"
reported=$(npm_lookup)
[ "$reported" != absent ] || reported=-
python3 "$here/registry_state.py" npm-plan "$manifest" "$dir/$tarball" "$reported" > "$work/npm-plan"
cat "$work/pypi-plan" "$work/npm-plan"
pending_python=()
while read -r action name; do [ "$action" = upload ] && pending_python+=("$dir/$name"); done < "$work/pypi-plan"
read -r npm_action _ < "$work/npm-plan"
readonly pending_python npm_action   # the plan printed above is what gets uploaded; nothing below may change it

echo "== credentials"
token_source=environment
if [ -f "$root/.env" ]; then
  git check-ignore -q "$root/.env" || { echo ".env is not ignored by git: add /.env to .gitignore before it gets committed" >&2; exit 1; }
  python3 -I -c 'import os, sys; sys.exit(1 if os.stat(sys.argv[1]).st_mode & 0o077 else 0)' "$root/.env" \
    || { echo ".env is readable by other users: chmod 600 .env" >&2; exit 1; }
  n=0
  while IFS= read -r line || [ -n "$line" ]; do
    n=$((n + 1))
    case "$line" in ''|'#'*) continue ;; esac
    line=${line#export }
    case "${line%%=*}" in   # only the one credential name; nothing else may be set from the file
      UV_PUBLISH_TOKEN) ;;
      *) echo ".env line $n: only UV_PUBLISH_TOKEN=<token> and comments are accepted" >&2; exit 1 ;;
    esac
    value=${line#*=}
    case "$value" in \"*\"|\'*\') value=${value:1:${#value}-2} ;; esac   # one layer of matching quotes
    if [ -z "${UV_PUBLISH_TOKEN:-}" ]; then
      export UV_PUBLISH_TOKEN="$value"
      token_source=.env
    fi
  done < "$root/.env"
fi
if [ "${#pending_python[@]}" -gt 0 ]; then
  [ -n "${UV_PUBLISH_TOKEN:-}" ] || { echo "set UV_PUBLISH_TOKEN (a PyPI token scoped to $pypi_project), in the environment or in $root/.env, to upload ${#pending_python[@]} Python file(s)" >&2; exit 1; }
  echo "PyPI token present ($token_source)"
fi
if [ "$npm_action" = upload ]; then
  npm_user=$(npm whoami --registry "$npm_registry" 2>/dev/null) || { echo "npm login --registry $npm_registry first (publishing $tarball)" >&2; exit 1; }
  echo "npm user $npm_user"
fi
if [ "$dry_run" = --dry-run ]; then
  echo "dry run: ${#pending_python[@]} Python file(s) to upload to $pypi_url, npm tarball: $npm_action; nothing uploaded"
  exit 0
fi

if [ "${#pending_python[@]}" -gt 0 ]; then
  echo "== PyPI: ${pending_python[*]}"
  uv publish --publish-url "$pypi_upload_url" --check-url "$pypi_url/simple/" "${pending_python[@]}"
else
  echo "== PyPI: nothing to upload"
fi
if [ "$npm_action" = upload ]; then
  echo "== npm: $tarball"
  npm publish "$dir/$tarball" --registry "$npm_registry" --access public
else
  echo "== npm: nothing to upload"
fi
echo "published $version ($commit, receipt on tag object $revision); next: scripts/release/verify-registries.sh $version on each platform"
