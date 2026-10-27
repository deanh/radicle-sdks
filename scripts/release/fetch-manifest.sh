#!/usr/bin/env bash
# Fetch and verify a release's MANIFEST.json through rad-artifact into the
# current directory, requiring exactly one release bound to the tag with
# exactly one live MANIFEST.json. Leaves listing.json, release.txt (tag object
# and CID of the manifest, then one line per live artifact: tag object, CID,
# name) and MANIFEST.json behind for the caller.
#
#   scripts/release/fetch-manifest.sh <version> <rid>
set -euo pipefail
version=${1:?usage: fetch-manifest.sh <version> <rid>}
rid=${2:?usage: fetch-manifest.sh <version> <rid>}
tag="v$version"
rad-artifact node status >/dev/null 2>&1 || { echo "start the seeder first: rad-artifact node start" >&2; exit 1; }
rad-artifact --repo "$rid" list --json > listing.json
python3 - "$tag" > release.txt <<'PY'
import json, sys
tag = sys.argv[1]
releases = [r for r in json.load(open("listing.json"))["releases"] if r.get("tagName") == tag]
if len(releases) != 1:
    sys.exit(f"{len(releases)} releases are bound to {tag}; expected exactly one")
r = releases[0]
live = [a for a in r["artifacts"] if not a["redactions"]]
manifests = [a for a in live if a["name"] == "MANIFEST.json"]
if len(manifests) != 1:
    sys.exit(f"release {tag} has {len(manifests)} live MANIFEST.json artifacts; expected exactly one")
print(r["tag"], manifests[0]["cid"])
for a in live:
    print(r["tag"], a["cid"], a["name"])
PY
read -r revision manifest_cid < <(head -1 release.txt)
rad-artifact --repo "$rid" --no-input download "$revision" --cid "$manifest_cid" -o MANIFEST.json >/dev/null
rad-artifact --repo "$rid" verify MANIFEST.json --json >/dev/null
python3 - "$version" <<'PY'
import json, sys
m = json.load(open("MANIFEST.json"))
if m["version"] != sys.argv[1]:
    sys.exit(f"MANIFEST.json is for {m['version']}, expected {sys.argv[1]}")
PY
