#!/usr/bin/env bash
# Register and seed one version's release artifacts on this repository's
# release object (rad-artifact), bound to the annotated tag v<version>.
#
#   scripts/release/register.sh <version>
#
# Preflight, each fatal and all before any write:
# - release/<version>/MANIFEST.json exists, names this version and exactly the
#   expected files; every file's SHA-256 matches the manifest and SHA256SUMS;
# - the local v<version> is an annotated tag whose object equals the one on
#   the rad remote, and that tag's commit equals the manifest's source;
# - the release state on this tag is compatible: at most one release object is
#   bound to the tag; every live artifact it holds has an expected name and
#   the same content hash (BLAKE3 CID) as the local file of that name. A live
#   artifact with an expected name but different content, or with an
#   unexpected name, is refused: published versions are immutable, so publish
#   a new version instead (or redact deliberately, by hand, first).
# Artifacts already present with identical content are resumed (not
# re-registered, still seeded); the rest are registered and seeded. The
# manifest is deterministic for identical artifact bytes, so a clean rebuild
# of the same source resumes rather than conflicting.
set -euo pipefail
version=${1:?usage: register.sh <version>}
root=$(git rev-parse --show-toplevel)
cd "$root"
dir="release/$version"
tag="v$version"
manifest="$dir/MANIFEST.json"
[ -f "$manifest" ] || { echo "missing $manifest; run build-linux-x86_64.sh first" >&2; exit 1; }
read -r m_version m_source m_target < <(python3 -c '
import json, sys
m = json.load(open(sys.argv[1]))
print(m["version"], m["source"], m["target"])' "$manifest")
[ "$m_version" = "$version" ] || { echo "manifest is for version $m_version, not $version" >&2; exit 1; }
[ "$m_target" = "x86_64-unknown-linux-gnu" ] || { echo "unexpected target $m_target" >&2; exit 1; }
expected=("radicle_py-$version-cp39-abi3-manylinux_2_28_x86_64.whl" "radicle_py-$version.tar.gz" "radicle-sdk-$version.tgz")
python3 - "$manifest" "${expected[@]}" <<'PY'
import hashlib, json, sys
from pathlib import Path
manifest = Path(sys.argv[1]); expected = sorted(sys.argv[2:])
m = json.load(manifest.open())
listed = sorted(m["files"])
if listed != expected:
    sys.exit(f"manifest lists {listed}, expected {expected}")
for name, entry in m["files"].items():
    p = manifest.parent / name
    if not p.is_file():
        sys.exit(f"missing artifact {name}")
    digest = hashlib.sha256(p.read_bytes()).hexdigest()
    if digest != entry["sha256"]:
        sys.exit(f"{name}: sha256 {digest} differs from manifest {entry['sha256']}; rebuild before registering")
    if p.stat().st_size != entry["bytes"]:
        sys.exit(f"{name}: size differs from manifest")
PY
(cd "$dir" && sha256sum -c --quiet SHA256SUMS) || { echo "SHA256SUMS does not match the files" >&2; exit 1; }
revision=$(git rev-parse --verify -q "refs/tags/$tag") || { echo "no local tag $tag" >&2; exit 1; }
[ "$(git cat-file -t "$revision")" = tag ] || { echo "$tag is not an annotated tag" >&2; exit 1; }
remote=$(git ls-remote --tags rad "refs/tags/$tag" | awk '$2 == "refs/tags/'"$tag"'" {print $1}')
[ -n "$remote" ] || { echo "push the tag first: git push rad $tag" >&2; exit 1; }
[ "$remote" = "$revision" ] || { echo "remote $tag is $remote, local is $revision; they must be the same tag object" >&2; exit 1; }
commit=$(git rev-parse "$revision^{commit}")
[ "$commit" = "$m_source" ] || { echo "$tag points at $commit but the artifacts were built from $m_source; rebuild from the tagged commit" >&2; exit 1; }
rid=$(rad .)
rad-artifact node status >/dev/null 2>&1 || { echo "start the seeder first: rad-artifact node start" >&2; exit 1; }

# Release-state preflight: local content hashes versus what the tag already holds.
names=("${expected[@]}" MANIFEST.json)
: > "$dir/.local-cids"
for name in "${names[@]}"; do
  printf '%s %s\n' "$(rad-artifact cid "$dir/$name")" "$name" >> "$dir/.local-cids"
done
rad-artifact --repo "$rid" list --json > "$dir/.listing.json"
python3 - "$dir/.listing.json" "$dir/.local-cids" "$revision" > "$dir/.plan" <<'PY'
import json, sys
listing, local_cids, revision = sys.argv[1:4]
local = dict(line.split()[::-1] for line in open(local_cids).read().splitlines())  # name -> cid
releases = [r for r in json.load(open(listing))["releases"] if r.get("tag") == revision]
if len(releases) > 1:
    sys.exit(f"{len(releases)} release objects are bound to this tag; refusing to add to an ambiguous release")
live = {}
for r in releases:
    for a in r["artifacts"]:
        if not a["redactions"]:
            live.setdefault(a["name"], set()).add(a["cid"])
conflicts = []
for name, cids in live.items():
    if name not in local:
        conflicts.append(f"{name}: live artifact with an unexpected name ({', '.join(sorted(cids))})")
    elif cids != {local[name]}:
        conflicts.append(f"{name}: live content {', '.join(sorted(cids))} differs from the local build {local[name]}")
if conflicts:
    sys.exit("release state on this tag conflicts with the local build; published versions are immutable:\n  "
             + "\n  ".join(conflicts) + "\nPublish a new version, or redact deliberately by hand before retrying.")
for name, cid in local.items():
    print("resume" if name in live else "register", name, cid)
PY
echo "registering $version ($commit, tag object $revision) on $rid"
release=""
while read -r action name cid; do
  file="$dir/$name"
  if [ "$action" = register ]; then
    json=$(rad-artifact --repo "$rid" --no-input register "$file" --revision "$revision" --name "$name" --json)
    got=$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["cid"])')
    [ "$got" = "$cid" ] || { echo "$name: registered CID $got differs from local $cid" >&2; exit 1; }
    release=$(printf '%s' "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["releaseId"])')
    printf 'registered %s  %s\n' "$cid" "$name"
  else
    printf 'already registered %s  %s\n' "$cid" "$name"
  fi
done < "$dir/.plan"
if [ -z "$release" ]; then
  release=$(python3 -c 'import json,sys; r=[r for r in json.load(open(sys.argv[1]))["releases"] if r.get("tag")==sys.argv[2]]; print(r[0]["releaseId"])' "$dir/.listing.json" "$revision")
fi
for name in "${names[@]}"; do
  rad-artifact --repo "$rid" --no-input seed "$dir/$name" --release "$release" >/dev/null
done
rm -f "$dir/.local-cids" "$dir/.listing.json" "$dir/.plan"
printf 'release %s bound to %s (%s)\n' "$release" "$tag" "$revision"
rad-artifact --repo "$rid" show --release "$release" --pretty
