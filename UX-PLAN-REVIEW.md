# Review of UX-PLAN.md

## Overall assessment

Good direction, but approve a smaller first slice—not U1–U3 wholesale. The plan identifies real friction and preserves the important architectural constraints. The main risk is that “Pythonic” becomes a new completeness target after deliberately choosing YAGNI.

## 1. U1 is not entirely additive or non-breaking

Adding equality changes existing behavior: distinct snapshots may collapse into one dictionary key or set entry. That is probably desirable, but needs an explicit contract.

Recommendations:

- Structural equality for immutable snapshots, including their contents—not just object IDs.
- Identity equality for handles, as proposed.
- Hashing only where useful, rather than automatically making every value hashable.
- Tests proving that modifying returned list copies cannot change equality/hash behavior.

P1 is slightly inaccurate: objects already work as dictionary keys/set members, but with identity semantics rather than useful value semantics.

Making classes `frozen` alone does not implement Python value semantics; most existing values already expose read-only properties.

## 2. Accept PathLike inputs; retain string returns

Accepting `PathBuf` is a straightforward improvement supported by the pinned PyO3 version. Update stubs to accept `str | os.PathLike[str]`, and decide explicitly whether byte paths are supported.

Do not make D2's proposed return-type change for now. Accepting PathLike inputs while returning strings is a reasonable Python API, not “half-Pythonic.” Changing returns adds migration work without fixing a demonstrated problem.

## 3. Specify datetime behavior precisely

The opening says UTC, but commit properties would retain their original offset. Distinguish these explicitly:

- Comments/reviews/revisions/raw operations/routing: aware UTC.
- Commit author/committer times: aware fixed-offset datetimes.
- Missing routing timestamp: `None`.
- Out-of-range timestamps: a documented exception, while raw integers remain accessible.
- Millisecond conversion: avoid unnecessary floating-point precision loss.

Keep existing abi3/Python 3.9 support intact.

## 4. History iteration needs correctness and performance caveats

Pinning the first page's resolved OID is correct, but:

- Advance by the number of results actually returned, not `page_size`: `history()` currently clamps limits to 1,000.
- Reopening and skipping increasing offsets repeatedly walks the same history. A full traversal can become quadratic.

This is acceptable for convenience over modest histories if documented. Pinning the head prevents ref movement; it does not establish a transactional snapshot or protect against missing objects.

Defer automatic traversal until a consumer needs it.

## 5. Context managers deserve a separate patch

Closing stores requires changing their retained-backend representation and checking accesses throughout the implementation. It is not merely mechanical.

Specify:

- `close()` is idempotent.
- `__exit__` closes without suppressing the body's exception.
- Metadata such as `path` remains readable after close.
- Whether reopening/re-entering a closed handle is rejected.
- No implicit transaction semantics.

Useful if callers need deterministic connection lifetime; implement separately from the broad UX bundle.

## 6. Narrow U2

### Keyword-only arguments

Worthwhile, especially `base`/`head` and project metadata. A deliberate breaking cleanup during alpha is reasonable.

### ReadOnlyError

Reasonable and focused.

### Generalized NotFoundError

Defer. It crosses several upstream error families and changes what existing domain-error handlers catch. If implemented, preserve useful distinctions such as repository versus Git-object failures and map structured variants only.

### Removing comment_count/comment_ids

Do not remove these now. Redundancy alone is not a defect, and these are convenient accessors. Removal buys little while creating churn.

## 7. Moderate U3's claims

- `stubtest` is useful, but cannot prove that returned runtime strings obey declared `Literal` types. Keep behavioral tests.
- `pyright --verifytypes` measures annotation completeness, not semantic correctness. Treat it as an additional check, not a completeness objective.
- Testing the actual README example is worthwhile. A pytest test extracting and executing the fenced example is more suitable than ordinary doctest for the current Markdown format.

## Recommended sequence

1. First patch: PathLike inputs, Literal annotations, collaboration reprs, `Profile.storage`, issue close/reopen sugar, and cheap container protocols.
2. Second patch: explicitly designed snapshot equality/hash semantics.
3. Separate patches when needed: deterministic store closure and datetime conveniences.
4. One small breaking cleanup: keyword-only parameters, possibly ReadOnlyError.
5. Defer: path return changes, accessor removal, generalized missing-object errors, and automatic history traversal.

The proposed 1–2 days / low-risk estimate for all of U1 is optimistic, particularly for equality across nested snapshots and connection lifecycle changes.

## Bottom line

Retain the guardrails, but prioritize concrete friction over universal Pythonic coverage.
