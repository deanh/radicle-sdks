# Documentation

## Current guides and contracts

- [Repository overview and installation](../README.md): Python and TypeScript
  entry points, source-install requirements and scope.
- [Python API](API.md): public API, conversion rules and upstream semantics.
- [Python changelog](../CHANGELOG.md).
- [TypeScript guide](../node/README.md): installation, executable quick start,
  public contract, concurrency and environment boundaries.
- [TypeScript changelog](../node/CHANGELOG.md).
- [Examples](../examples/README.md): the read-only digest in both SDKs.

## Historical plans, reviews and findings

The files in `archive/` are retained as implementation/review evidence, **not
current release checklists or support guarantees**. Dates, test counts, early
proposals and old filenames in their prose describe the work at that time.
Use the current guides above for supported behavior. Historical references to
other local checkouts are not dependencies of this repository.

### Python

- [Original implementation plan](archive/PLAN.md).
- [Alpha scope and completion record](archive/ALPHA-PLAN.md).
- [UX plan](archive/UX-PLAN.md) and [review](archive/UX-PLAN-REVIEW.md).
- Findings: [feasibility](archive/FINDINGS-0A.md),
  [repository reads](archive/FINDINGS-02.md),
  [policy/routing](archive/FINDINGS-03.md),
  [COB readers](archive/FINDINGS-04.md),
  [writes](archive/FINDINGS-05.md).

### TypeScript

- [SDK proposal](archive/TS-SDK-PROPOSAL.md) and
  [review](archive/TS-SDK-PROPOSAL-REVIEW.md).
- Findings: [T0 foundation](archive/FINDINGS-TS-T0.md),
  [repository operations](archive/FINDINGS-TS-REPOSITORY.md),
  [alpha release builds](archive/FINDINGS-TS-ALPHA-RELEASE.md),
  [alpha API and executable docs](archive/FINDINGS-TS-ALPHA-API-DOCS.md).
