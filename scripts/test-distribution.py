#!/usr/bin/env python3

import re
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


manifest = tomllib.loads((ROOT / "packslip.toml").read_text())
require(manifest.get("bin") == ["kakitsugi"], "packslip bin must be kakitsugi")

artifacts = {item.get("path"): item for item in manifest.get("artifact", [])}
require(
    set(artifacts)
    == {
        "dist/kakitsugi-aarch64-apple-darwin.tar.gz",
        "dist/kakitsugi-x86_64-unknown-linux-gnu.tar.gz",
    },
    "packslip artifacts must match the two release archives",
)
require(
    artifacts["dist/kakitsugi-aarch64-apple-darwin.tar.gz"].get("requires") is None,
    "macOS artifact must not declare Linux requirements",
)
require(
    artifacts["dist/kakitsugi-x86_64-unknown-linux-gnu.tar.gz"].get("requires")
    == {"glibc_min": "2.39"},
    "Linux artifact must require glibc 2.39",
)

resources = manifest.get("resource", [])
require(
    resources
    == [{"kind": "skill", "name": "kakitsugi", "repo": "skills/kakitsugi"}],
    "packslip must publish the kakitsugi skill from the repository",
)

skill_dir = ROOT / resources[0]["repo"]
skill_text = (skill_dir / "SKILL.md").read_text()
frontmatter = re.match(r"\A---\n(?P<body>.*?)\n---\n", skill_text, re.DOTALL)
require(frontmatter is not None, "SKILL.md must have YAML frontmatter")
require(
    re.search(r"(?m)^name:\s*kakitsugi\s*$", frontmatter["body"]) is not None,
    "SKILL.md frontmatter name must be kakitsugi",
)
require(
    re.search(r"(?m)^description:\s*\S.+$", frontmatter["body"]) is not None,
    "SKILL.md frontmatter must have a description",
)

openai_text = (skill_dir / "agents" / "openai.yaml").read_text()
require('display_name: "Kakitsugi"' in openai_text, "OpenAI metadata name is missing")
require(
    re.search(
        r"(?ms)^dependencies:\s*\n\s+tools:\s*\n\s+- type:\s*[\"']?mcp[\"']?\s*\n"
        r"\s+value:\s*[\"']?kakitsugi[\"']?\s*$",
        openai_text,
    )
    is not None,
    "OpenAI metadata must require the kakitsugi MCP server",
)

print("distribution contract test passed")
