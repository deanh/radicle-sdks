"""Execute the actual README fenced Python example, not a copied example."""
import os
from pathlib import Path
import re
import subprocess
import sys


def test_readme_quick_start(tmp_path):
    readme = (Path(__file__).resolve().parents[1] / "README.md").read_text()
    section = readme.split("## Runnable local quick start", 1)[1].split("\n## ", 1)[0]
    blocks = re.findall(r"```python\n(.*?)\n```", section, re.DOTALL)
    assert len(blocks) == 1
    script = tmp_path / "quick_start.py"
    script.write_text(blocks[0])
    env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
    result = subprocess.run([sys.executable, str(script)], cwd=tmp_path, env=env,
                            capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, result.stdout + result.stderr
