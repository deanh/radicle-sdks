// Project digest: a read-only Markdown report of your local Radicle projects.
//
// For each project it lists patches in progress (with the latest revision's
// review verdicts and whether it still needs *your* review), merged patches,
// open issues grouped by label, and what is assigned to you. This is structured
// data the `rad` CLI only offers as text.
//
// Usage:
//     node examples/digest.mjs [RID ...]
//
// Reads the profile from RAD_HOME (default ~/.radicle). With no RIDs, covers
// every repository in local storage. It never loads a signer and never writes.
//
// `examples/digest.py` is the same program for the Python SDK; the two must
// print identical output (checked by tests/test_examples.py).

import { Profile, Issues, Patches, RadicleError } from 'radicle-sdk';

const short = (oid) => oid.slice(0, 7);
// Locale-independent Unicode code-point order, matching Python's string order.
// JS `<` compares UTF-16 code units, which misorders characters above U+FFFF
// (e.g. an emoji sorts before U+FF21). UTF-8 byte order equals code-point order
// for valid Unicode, which SDK strings always are (they come from Rust).
const compare = (a, b) => Buffer.compare(Buffer.from(a, 'utf8'), Buffer.from(b, 'utf8'));
const byTitle = (items) =>
  [...items].sort((a, b) => compare(a.title, b.title) || compare(a.id, b.id));

class Digest {
  constructor(profile) {
    this.me = profile.did;
    this.storage = profile.storage;
    this.lines = [];
  }

  who(did) {
    return did === this.me ? 'you' : `did:key:...${did.slice(-8)}`;
  }

  emit(line = '') {
    this.lines.push(line);
  }

  async run(rids) {
    this.emit('# Radicle digest');
    this.emit();
    this.emit(`Profile: ${this.me}`);
    if (rids.length === 0) {
      this.emit();
      this.emit("No repositories in this profile's storage.");
    }
    for (const rid of rids) {
      this.emit();
      const mark = this.lines.length;
      try {
        await this.project(this.storage.repository(rid));
      } catch (error) {
        if (!(error instanceof RadicleError)) throw error;
        this.lines.length = mark; // drop a partially written section
        this.emit(`## ${rid}`);
        this.emit();
        this.emit('_Could not read this repository._');
      }
    }
    return this.lines.join('\n') + '\n';
  }

  async project(repo) {
    const doc = await repo.identity();
    this.emit(`## ${doc.name} (${repo.rid})`);
    if (doc.description) {
      this.emit();
      this.emit(doc.description);
    }
    this.emit();
    this.emit(`Default branch \`${doc.defaultBranch}\` at \`${short((await repo.head()).oid)}\`.`);
    const patches = await Patches.open(repo).all();
    const issues = await Issues.open(repo).all();
    this.patchesInProgress(patches);
    await this.mergedPatches(repo, patches);
    this.openIssues(issues);
    this.assigned(issues, patches);
  }

  patchesInProgress(patches) {
    this.emit();
    this.emit('### Patches in progress');
    this.emit();
    const active = byTitle(patches.filter((p) => p.state === 'open' || p.state === 'draft'));
    if (active.length === 0) this.emit('_None._');
    for (const patch of active) {
      const latest = patch.revisions.find((r) => r.id === patch.latestRevision);
      if (latest === undefined) {
        // Only a redacted author revision can be missing from the list.
        this.emit(
          `- **${patch.title}** \`${short(patch.id)}\` (${patch.state}), ` +
            `by ${this.who(patch.author)}: latest revision unavailable.`,
        );
        continue;
      }
      const verdicts = [...latest.reviews]
        .sort((a, b) => compare(a.author, b.author))
        .filter((r) => r.verdict)
        .map((r) => `${r.verdict} from ${this.who(r.author)}`);
      let line =
        `- **${patch.title}** \`${short(patch.id)}\` (${patch.state}), ` +
        `by ${this.who(patch.author)}, ${patch.revisions.length} revision(s); ` +
        `latest revision verdicts: ${verdicts.join('; ') || 'none yet'}.`;
      const reviewed = latest.reviews.some((r) => r.author === this.me);
      if (patch.state === 'open' && patch.author !== this.me && !reviewed) {
        line += ' **Needs your review.**';
      }
      this.emit(line);
    }
  }

  async mergedPatches(repo, patches) {
    this.emit();
    this.emit('### Merged patches');
    this.emit();
    const merged = byTitle(patches.filter((p) => p.state === 'merged'));
    if (merged.length === 0) this.emit('_None._');
    for (const patch of merged) {
      let summary;
      try {
        summary = (await repo.commit(patch.mergedCommit)).messageStr.split('\n')[0];
      } catch (error) {
        if (!(error instanceof RadicleError)) throw error;
        summary = '(commit unavailable)';
      }
      this.emit(
        `- **${patch.title}** \`${short(patch.id)}\` merged as ` +
          `\`${short(patch.mergedCommit)}\`: ${summary}`,
      );
    }
  }

  openIssues(issues) {
    this.emit();
    this.emit('### Open issues');
    this.emit();
    const open = issues.filter((i) => i.state === 'open');
    if (open.length === 0) {
      this.emit('_None._');
      return;
    }
    const labels = [...new Set(open.flatMap((i) => i.labels))].sort(compare);
    const groups = labels.map((label) => [label, open.filter((i) => i.labels.includes(label))]);
    groups.push(['(no label)', open.filter((i) => i.labels.length === 0)]);
    for (const [label, group] of groups) {
      if (group.length === 0) continue;
      this.emit(`#### ${label}`);
      this.emit();
      for (const issue of byTitle(group)) {
        let line = `- **${issue.title}** \`${short(issue.id)}\`, ${issue.comments.length - 1} comment(s)`;
        if (issue.assignees.includes(this.me)) line += ', assigned to you';
        this.emit(line);
      }
      this.emit();
    }
    this.lines.pop(); // no blank line after the last group
  }

  assigned(issues, patches) {
    this.emit();
    this.emit('### Assigned to you');
    this.emit();
    const mine = [
      ...byTitle(issues)
        .filter((i) => i.state === 'open' && i.assignees.includes(this.me))
        .map((i) => ['issue', i]),
      ...byTitle(patches)
        .filter((p) => (p.state === 'open' || p.state === 'draft') && p.assignees.includes(this.me))
        .map((p) => ['patch', p]),
    ];
    if (mine.length === 0) this.emit('_Nothing._');
    for (const [kind, item] of mine) this.emit(`- ${kind} **${item.title}** \`${short(item.id)}\``);
  }
}

const profile = Profile.load();
const args = process.argv.slice(2);
const rids = args.length > 0 ? args : [...(await profile.storage.repositories())].sort(compare);
process.stdout.write(await new Digest(profile).run(rids));
