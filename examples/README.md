# Examples

Small programs showing what the SDKs make possible. They use only public APIs
that both the Python (`radiclepy`) and TypeScript (`radicle-sdk`) SDKs provide,
and they are executed by the test suite so they can't silently rot.

Planned in Radicle issue `6595eea`: this read-only digest first, then a
write-side example (a review bot in a sandbox), since signed local writes are
what read-only tooling such as `radicle-httpd` cannot do.

## Project digest (read-only)

[`digest.py`](digest.py) and [`digest.mjs`](digest.mjs) are the same program in
each SDK. For each project in your local storage they print a Markdown report:

- **Patches in progress** (open and draft), with the latest revision's review
  verdicts, flagging open patches by others that still need *your* review.
- **Merged patches**, with the merge commit and its summary line.
- **Open issues** grouped by label (an issue with several labels appears under
  each), unlabeled last.
- **Assigned to you**: open issues and in-progress patches.

The `rad` CLI only offers this information as human-oriented text; here it comes
from typed snapshots. The digest **never loads a signer and never writes**, so it
is safe to run against your real profile.

```sh
# Python (after installing radiclepy)
python examples/digest.py                 # every repository in storage
python examples/digest.py rad:z...        # just these repositories

# TypeScript (after `npm run build` in this checkout, or with radicle-sdk installed)
node examples/digest.mjs
node examples/digest.mjs rad:z...
```

Both read the profile from `RAD_HOME` (default `~/.radicle`) and print identical
output; `tests/test_examples.py` builds one fixture and checks that byte for byte.
A repository that can't be read is reported as such rather than aborting the run.

Sample output (abridged):

```markdown
## sdk-test (rad:z2BA5LVLaJB4LpiyHPn8pKAhJhgTG)

Default branch `main` at `d774ba0`.

### Patches in progress

- **Add Bun notes** `d1c3480` (open), by did:key:...UtNVnt4o, 1 revision(s); latest revision verdicts: none yet. **Needs your review.**

### Merged patches

- **Speed up history paging** `61a153a` merged as `244a63d`: Speed up paging

### Open issues

#### bug

- **Crash on empty repo** `048b03a`, 1 comment(s), assigned to you

### Assigned to you

- issue **Crash on empty repo** `048b03a`
```

Other actors are shown by the last eight characters of their DID; the SDKs'
alias lookup isn't used because only the Python SDK has it.
