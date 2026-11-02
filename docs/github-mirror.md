# GitHub mirror

The canonical repository is Radicle. GitHub is a read-only distribution mirror:

- Canonical Radicle repository: `rad:z27dDy6sppkZDG8qYcySVX9Qn5jNg`
- GitHub mirror: `git@github.com:deanh/radicle-sdks.git`

Do not merge GitHub pull requests or treat GitHub branch state as authoritative.
Issues and patches belong on Radicle unless a later project policy says otherwise.

## Configure the remote

The remote is local Git configuration and is not stored in the repository:

```sh
git remote add github git@github.com:deanh/radicle-sdks.git
```

Before first SSH use, configure and verify GitHub's host key using GitHub's
published fingerprints. Use a dedicated deploy key or other least-privilege
credential for unattended mirroring.

## Mirror canonical refs

Run the versioned script after changes have landed on canonical Radicle `main`:

```sh
scripts/mirror-github.sh --dry-run
scripts/mirror-github.sh
```

The script fetches both remotes, then atomically pushes:

- `refs/remotes/rad/main` to GitHub `refs/heads/main`;
- Radicle release refs under `refs/remotes/rad/tags/*` to matching GitHub tags.

It does not push local topic branches, Radicle patches, namespaces, remote-tracking
refs, or internal refs. It never force-pushes or deletes GitHub refs. Divergence
or a conflicting tag fails for manual inspection instead of rewriting history.
After a real push it verifies that both `main` OIDs match.

Environment variables can change the configured remote or branch names:

```sh
RADICLE_MIRROR_SOURCE_REMOTE=rad \
RADICLE_MIRROR_TARGET_REMOTE=github \
RADICLE_MIRROR_BRANCH=main \
  scripts/mirror-github.sh
```

Use `--no-fetch` only when a caller has already fetched both remotes and intends
to mirror exactly those local remote-tracking refs.

## Automation

Do not make this an authoritative `post-commit` hook: a local commit is not proof
that a change has landed canonically, hooks are machine-local, and network errors
should not break normal commits.

Start with an explicit mirror step after landing and release operations. For
unattended mirroring, run the same script from one controlled host using a
systemd timer, cron job, or self-hosted CI runner. Alert on failure; do not add an
automatic force or retry policy for divergent refs.
