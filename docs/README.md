# Documentation

## Shared documentation

- [Project overview](../README.md): SDK scope and language entry points.
- [Architecture](architecture.md): current layers, ownership and migration boundaries.
- [Workflow semantics](semantics.md): signing, Git/COB writes, revisions and merges.
- [SDK differences](sdk-differences.md): intentional API/conversion/concurrency differences.
- [TypeScript parity checklist](node-parity.md): three-patch capability-parity plan and completion gates.
- [Development and validation](development.md): commands, artifacts and evidence gates.
- [Releasing](release.md): building release artifacts on the maintainer's machines and
  distributing them through rad-artifact.
- [Events contract](events.md): read-only primitives, change records and cursor text
  for the planned repository event feed (draft, issue `0896069`).
- [Examples](../examples/README.md): the read-only digest in both SDKs.

## Language guides and contracts

Each SDK owns its guide, reference and release history in its language directory.

- Python: [agent quick reference](../python/AGENT.md),
  [guide](../python/README.md), [API](../python/API.md),
  [changelog](../python/CHANGELOG.md).
- TypeScript: [agent quick reference](../node/AGENT.md),
  [guide](../node/README.md), [API](../node/API.md),
  [changelog](../node/CHANGELOG.md).

The onboarding rationale and consumer evidence are recorded in the
[Python assessment](python-agent-onboarding-assessment.md) and
[TypeScript assessment](typescript-agent-onboarding-assessment.md).

## Historical plans, reviews and findings

The files in `archive/` are retained as implementation/review evidence, **not
current release checklists or support guarantees**. Dates, test counts, early
proposals and old filenames in their prose describe the work at that time.
Use the current guides above for supported behavior. Historical references to
other local checkouts are not dependencies of this repository.

### Historical source probes

- [Exact retrieval commit and commands](archive/PROBES.md) for the removed
  `spike/` and `wheel-probe/` sources. Their original findings below are preserved.

### Shared SDK structure

- [Module alignment findings](archive/FINDINGS-MODULE-ALIGNMENT.md): domain
  ownership, preservation checks and fresh artifact validation.
- [Workspace layout findings](archive/FINDINGS-WORKSPACE-LAYOUT.md): explicit
  dependency alignment, feature isolation and source-packaging evidence.
- [Migration gates](archive/MIGRATION-GATES.md): the validation gates,
  dependency audit and pre-alignment module map for the completed migration.

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
