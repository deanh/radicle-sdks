"""Keep repository-local links and #anchors valid when guides and historical docs move."""
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
PAGES = [ROOT / "README.md", ROOT / "examples" / "README.md",
         *sorted((ROOT / "python").glob("*.md")),
         *sorted((ROOT / "node").glob("*.md")),
         *sorted((ROOT / "docs").rglob("*.md"))]


def anchors(page: Path) -> set[str]:
    """GitHub-style heading anchors: lowercase, punctuation dropped, spaces to
    hyphens, duplicates suffixed -1, -2, ... Headings inside code fences are skipped."""
    seen: dict[str, int] = {}
    result: set[str] = set()
    fenced = False
    for line in page.read_text().splitlines():
        if line.lstrip().startswith("```"):
            fenced = not fenced
            continue
        match = None if fenced else re.match(r"#{1,6}\s+(.*?)\s*#*\s*$", line)
        if not match:
            continue
        slug = re.sub(r"[^\w\- ]", "", match.group(1).strip().lower()).replace(" ", "-")
        count = seen.get(slug, 0)
        seen[slug] = count + 1
        result.add(slug if count == 0 else f"{slug}-{count}")
    return result


def test_documentation_local_links_and_anchors():
    missing = []
    for page in PAGES:
        for link in re.findall(r"\]\(([^)\s]+)\)", page.read_text()):
            url = urlsplit(link)
            if url.scheme or url.netloc:
                continue
            target = (page.parent / unquote(url.path)).resolve() if url.path else page
            # Archived research also cites sibling developer checkouts.
            if not target.is_relative_to(ROOT):
                continue
            if not target.exists():
                missing.append(f"{page.relative_to(ROOT)}: {link}")
            elif url.fragment and target.suffix == ".md" and unquote(url.fragment) not in anchors(target):
                missing.append(f"{page.relative_to(ROOT)}: {link} (no such heading)")
    assert not missing, "Broken local documentation links:\n" + "\n".join(missing)


def test_language_guides_and_references_are_checked():
    for language in ("python", "node"):
        for name in ("AGENT.md", "README.md", "API.md", "CHANGELOG.md"):
            assert ROOT / language / name in PAGES
    assert 'readme = "python/README.md"' in (ROOT / "pyproject.toml").read_text()
    overview = (ROOT / "README.md").read_text()
    assert "```python" not in overview and "```js" not in overview
    assert not (ROOT / "CHANGELOG.md").exists()


def test_anchor_slugs_follow_github_rules(tmp_path):
    page = tmp_path / "page.md"
    page.write_text("# Python installation\n## `docs/API.md`: the contract!\n"
                    "## Notes\n## Notes\n```sh\n# not a heading\n```\n")
    assert anchors(page) == {"python-installation", "docsapimd-the-contract", "notes", "notes-1"}
