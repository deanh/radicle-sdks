#!/usr/bin/env bash
# Mirror canonical Radicle main and release tags to the read-only GitHub mirror.
# Radicle remains the source of truth. This script never force-pushes or deletes.
set -euo pipefail

source_remote="${RADICLE_MIRROR_SOURCE_REMOTE:-rad}"
target_remote="${RADICLE_MIRROR_TARGET_REMOTE:-github}"
branch="${RADICLE_MIRROR_BRANCH:-main}"
dry_run=0
fetch=1

usage() {
  cat <<'EOF'
Usage: scripts/mirror-github.sh [--dry-run] [--no-fetch]

Environment overrides:
  RADICLE_MIRROR_SOURCE_REMOTE   source remote (default: rad)
  RADICLE_MIRROR_TARGET_REMOTE   target remote (default: github)
  RADICLE_MIRROR_BRANCH          canonical branch (default: main)

Mirrors refs/remotes/<source>/<branch> and refs/remotes/<source>/tags/*.
The push is atomic, non-forced, and never deletes target refs.
EOF
}

while (($#)); do
  case "$1" in
    --dry-run) dry_run=1 ;;
    --no-fetch) fetch=0 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

git remote get-url "$source_remote" >/dev/null
git remote get-url "$target_remote" >/dev/null

if ((fetch)); then
  git fetch --prune "$source_remote"
  git fetch --prune "$target_remote"
fi

source_branch="refs/remotes/${source_remote}/${branch}"
if ! git show-ref --verify --quiet "$source_branch"; then
  echo "missing canonical source branch: $source_branch" >&2
  exit 1
fi

refspecs=("${source_branch}:refs/heads/${branch}")
while IFS= read -r tag_ref; do
  tag="${tag_ref#refs/remotes/${source_remote}/tags/}"
  refspecs+=("${tag_ref}:refs/tags/${tag}")
done < <(git for-each-ref --format='%(refname)' "refs/remotes/${source_remote}/tags/")

push_args=(push --atomic)
if ((dry_run)); then
  push_args+=(--dry-run)
fi
push_args+=("$target_remote" "${refspecs[@]}")

echo "Mirroring ${source_branch} and $((${#refspecs[@]} - 1)) tag(s) to ${target_remote}"
git "${push_args[@]}"

if ((!dry_run)); then
  source_oid="$(git rev-parse "$source_branch")"
  target_oid="$(git ls-remote "$target_remote" "refs/heads/$branch" | awk 'NR == 1 { print $1 }')"
  if [[ "$source_oid" != "$target_oid" ]]; then
    echo "mirror verification failed: source=$source_oid target=${target_oid:-missing}" >&2
    exit 1
  fi
  echo "Verified ${target_remote}/${branch} at ${source_oid}"
fi
