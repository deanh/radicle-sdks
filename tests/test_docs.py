"""Keep repository-local links valid when guides and historical docs move."""
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]


def test_documentation_local_links():
    pages = [ROOT / "README.md", ROOT / "CHANGELOG.md",
             ROOT / "node" / "README.md", ROOT / "node" / "CHANGELOG.md",
             ROOT / "examples" / "README.md", *sorted((ROOT / "docs").rglob("*.md"))]
    missing = []
    for page in pages:
        for link in re.findall(r"\]\(([^)\s]+)\)", page.read_text()):
            url = urlsplit(link)
            if url.scheme or url.netloc or not url.path:
                continue
            target = (page.parent / unquote(url.path)).resolve()
            # Archived research also cites sibling developer checkouts.
            if not target.is_relative_to(ROOT):
                continue
            if not target.exists():
                missing.append(f"{page.relative_to(ROOT)}: {link}")
    assert not missing, "Broken local documentation links:\n" + "\n".join(missing)
