"""Project digest: a read-only Markdown report of your local Radicle projects.

For each project it lists patches in progress (with the latest revision's review
verdicts and whether it still needs *your* review), merged patches, open issues
grouped by label, and what is assigned to you. This is structured data the `rad`
CLI only offers as text.

Usage:
    python examples/digest.py [RID ...]

Reads the profile from RAD_HOME (default ~/.radicle). With no RIDs, covers every
repository in local storage. It never loads a signer and never writes.

`examples/digest.mjs` is the same program for the TypeScript SDK; the two must
print identical output (checked by tests/test_examples.py).
"""

from __future__ import annotations

import sys

import radiclepy as rad


def short(oid: str) -> str:
    return oid[:7]


def by_title(items):
    # Unicode code-point order (Python's string order). The TypeScript port
    # compares UTF-8 bytes to get the same order; JS `<` would not.
    return sorted(items, key=lambda item: (item.title, item.id))


class Digest:
    def __init__(self, profile: rad.Profile) -> None:
        self.me = profile.did
        self.storage = profile.storage
        self.lines: list[str] = []

    def who(self, did: str) -> str:
        return "you" if did == self.me else f"did:key:...{did[-8:]}"

    def emit(self, line: str = "") -> None:
        self.lines.append(line)

    def run(self, rids: list[str]) -> str:
        self.emit("# Radicle digest")
        self.emit()
        self.emit(f"Profile: {self.me}")
        if not rids:
            self.emit()
            self.emit("No repositories in this profile's storage.")
        for rid in rids:
            self.emit()
            mark = len(self.lines)
            try:
                self.project(self.storage.repository(rid))
            except rad.Error:
                del self.lines[mark:]  # drop a partially written section
                self.emit(f"## {rid}")
                self.emit()
                self.emit("_Could not read this repository._")
        return "\n".join(self.lines) + "\n"

    def project(self, repo: rad.Repository) -> None:
        doc = repo.identity()
        self.emit(f"## {doc.name} ({repo.rid})")
        if doc.description:
            self.emit()
            self.emit(doc.description)
        self.emit()
        self.emit(f"Default branch `{doc.default_branch}` at `{short(repo.head.oid)}`.")
        patches = rad.Patches.open(repo).all()
        issues = rad.Issues.open(repo).all()
        self.patches_in_progress(patches)
        self.merged_patches(repo, patches)
        self.open_issues(issues)
        self.assigned(issues, patches)

    def patches_in_progress(self, patches) -> None:
        self.emit()
        self.emit("### Patches in progress")
        self.emit()
        active = by_title(p for p in patches if p.state in ("open", "draft"))
        if not active:
            self.emit("_None._")
        for patch in active:
            latest = next(r for r in patch.revisions if r.id == patch.latest_revision)
            verdicts = [f"{r.verdict} from {self.who(r.author)}"
                        for r in sorted(latest.reviews, key=lambda r: r.author) if r.verdict]
            line = (f"- **{patch.title}** `{short(patch.id)}` ({patch.state}), "
                    f"by {self.who(patch.author)}, {len(patch.revisions)} revision(s); "
                    f"latest revision verdicts: {'; '.join(verdicts) or 'none yet'}.")
            reviewed = any(r.author == self.me for r in latest.reviews)
            if patch.state == "open" and patch.author != self.me and not reviewed:
                line += " **Needs your review.**"
            self.emit(line)

    def merged_patches(self, repo: rad.Repository, patches) -> None:
        self.emit()
        self.emit("### Merged patches")
        self.emit()
        merged = by_title(p for p in patches if p.state == "merged")
        if not merged:
            self.emit("_None._")
        for patch in merged:
            try:
                summary = repo.commit(patch.merged_commit).message_str.split("\n")[0]
            except rad.Error:
                summary = "(commit unavailable)"
            self.emit(f"- **{patch.title}** `{short(patch.id)}` merged as "
                      f"`{short(patch.merged_commit)}`: {summary}")

    def open_issues(self, issues) -> None:
        self.emit()
        self.emit("### Open issues")
        self.emit()
        open_issues = [i for i in issues if i.state == "open"]
        if not open_issues:
            self.emit("_None._")
            return
        labels = sorted({label for i in open_issues for label in i.labels})
        groups = [(label, [i for i in open_issues if label in i.labels]) for label in labels]
        groups.append(("(no label)", [i for i in open_issues if not i.labels]))
        for label, group in groups:
            if not group:
                continue
            self.emit(f"#### {label}")
            self.emit()
            for issue in by_title(group):
                line = (f"- **{issue.title}** `{short(issue.id)}`, "
                        f"{len(issue.comments) - 1} comment(s)")
                if self.me in issue.assignees:
                    line += ", assigned to you"
                self.emit(line)
            self.emit()
        self.lines.pop()  # no blank line after the last group

    def assigned(self, issues, patches) -> None:
        self.emit()
        self.emit("### Assigned to you")
        self.emit()
        mine = ([("issue", i) for i in by_title(issues) if i.state == "open" and self.me in i.assignees]
                + [("patch", p) for p in by_title(patches)
                   if p.state in ("open", "draft") and self.me in p.assignees])
        if not mine:
            self.emit("_Nothing._")
        for kind, item in mine:
            self.emit(f"- {kind} **{item.title}** `{short(item.id)}`")


def main(argv: list[str]) -> int:
    profile = rad.Profile.load()
    rids = argv or sorted(profile.storage.repositories())
    sys.stdout.write(Digest(profile).run(rids))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
