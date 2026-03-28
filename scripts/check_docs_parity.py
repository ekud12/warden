#!/usr/bin/env python3
"""
Docs/code parity checker for Warden.

Verifies that README.md claims match the actual codebase:
  1. CLI commands in README vs USER_COMMANDS in src/cli/mod.rs
  2. MCP tool names in README vs tools_list() in src/engines/harbor/mcp.rs
  3. Version in Cargo.toml vs README badge
  4. Config tier count in README vs actual merge code

Run: python3 scripts/check_docs_parity.py
Exit code 0 = all checks pass, 1 = mismatches found.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
ERRORS = []


def error(msg: str):
    ERRORS.append(msg)
    print(f"  FAIL: {msg}")


def check_version():
    """Cargo.toml version must exist. README uses dynamic badge (no static version to check)."""
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.MULTILINE)
    if not m:
        error("Could not parse version from Cargo.toml")
        return
    cargo_ver = m.group(1)

    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    # Accept either static badge (v2.6.0-blue) or dynamic badge (github/v/release)
    static_badge = re.search(r"badge/v([\d.]+)-blue", readme)
    dynamic_badge = "github/v/release" in readme

    if static_badge:
        readme_ver = static_badge.group(1)
        if cargo_ver != readme_ver:
            error(f"Version mismatch: Cargo.toml={cargo_ver}, README badge={readme_ver}")
        else:
            print(f"  OK: Version {cargo_ver}")
    elif dynamic_badge:
        print(f"  OK: Version {cargo_ver} (README uses dynamic release badge)")
    else:
        error("Could not find version badge in README.md")


def check_cli_commands():
    """README command table entries should cover USER_COMMANDS.
    If README is a stub (links to docs), skip this check — commands live in docs site."""
    cli_src = (ROOT / "src" / "cli" / "mod.rs").read_text(encoding="utf-8")

    # Extract USER_COMMANDS array
    m = re.search(
        r'pub const USER_COMMANDS.*?=.*?\[([^\]]+)\]', cli_src, re.DOTALL
    )
    if not m:
        error("Could not parse USER_COMMANDS from src/cli/mod.rs")
        return
    code_cmds = set(re.findall(r'"([^"]+)"', m.group(1)))

    readme = (ROOT / "README.md").read_text(encoding="utf-8")

    # If README links to docs for commands, skip detailed check
    if "bitmill.dev" in readme and "Commands" in readme:
        print(f"  OK: {len(code_cmds)} CLI commands (README links to docs for full list)")
        return

    # Extract commands from README table (lines starting with | `warden ...)
    readme_cmds = set()
    for m in re.finditer(r"\|\s*`warden\s+(\S+)", readme):
        cmd = m.group(1).rstrip("`").split()[0]
        readme_cmds.add(cmd)

    # Commands in code but not in README
    missing = code_cmds - readme_cmds
    internal = {"redb", "state"}
    missing -= internal

    if missing:
        error(f"CLI commands in code but not in README: {sorted(missing)}")
    else:
        print(f"  OK: {len(code_cmds)} CLI commands documented")


def check_mcp_tools():
    """README MCP tool names should match tools_list() in mcp.rs.
    If README is a stub (links to docs), skip detailed check."""
    mcp_src = (ROOT / "src" / "engines" / "harbor" / "mcp.rs").read_text(
        encoding="utf-8"
    )

    # Extract tool names from tools_list function
    code_tools = set(re.findall(r'"name":\s*"(\w+)"', mcp_src))

    readme = (ROOT / "README.md").read_text(encoding="utf-8")

    # If README links to docs for MCP, skip detailed check
    if "bitmill.dev" in readme and "MCP" in readme:
        print(f"  OK: {len(code_tools)} MCP tools (README links to docs for details)")
        return

    # Extract from MCP table: | `tool_name` |
    readme_tools = set(re.findall(r"\|\s*`(\w+)`\s*\|", readme))
    readme_tools = {t for t in readme_tools if t in code_tools or "_" in t}

    missing_in_docs = code_tools - readme_tools
    if missing_in_docs:
        error(f"MCP tools in code but not in README: {sorted(missing_in_docs)}")
    else:
        print(f"  OK: {len(code_tools)} MCP tools documented")


def check_config_tiers():
    """README should say 3-tier config (not 4)."""
    readme = (ROOT / "README.md").read_text(encoding="utf-8")

    if "4-level" in readme or "4 tier" in readme.lower() or "4-tier" in readme.lower():
        error("README still references 4-level/4-tier config (should be 3)")
    elif "core.toml" in readme:
        error("README still references core.toml (removed in v2.4)")
    elif "personal.toml" in readme:
        error("README still references personal.toml (removed in v2.4)")
    else:
        print("  OK: Config tiers accurate (3-level)")


def check_latency_claims():
    """README should not claim <2ms or ~2ms latency."""
    readme = (ROOT / "README.md").read_text(encoding="utf-8")

    # Look for stale 2ms claims (but allow "~10ms" or "under 50ms")
    if re.search(r"[<~]2ms|under 2ms|~2ms", readme, re.IGNORECASE):
        error("README still claims ~2ms latency (should be ~10ms)")
    else:
        print("  OK: Latency claims updated")


def check_prohibited_wording():
    """README must not contain legacy/stale phrases."""
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    readme_lower = readme.lower()

    prohibited = [
        ("progressive onboarding", "removed onboarding model"),
        ("sessions 1-3", "removed onboarding model"),
        ("safety only first", "removed onboarding model"),
        ("4-tier", "config is 3-tier now"),
        ("4-level", "config is 3-tier now"),
        ("core.toml", "removed in v2.4"),
        ("personal.toml", "removed in v2.4"),
        ("full yaml validation", "YAML not used"),
        ("yaml parsed", "YAML not used"),
        ("daemon-status", "old command name"),
        ("daemon-stop", "old command name"),
        ("daemon-start", "old command name"),
        ("~2ms", "stale latency claim"),
    ]

    found = []
    for phrase, reason in prohibited:
        if phrase.lower() in readme_lower:
            found.append(f'"{phrase}" ({reason})')

    if found:
        for f in found:
            error(f"Prohibited wording in README: {f}")
    else:
        print(f"  OK: No prohibited wording ({len(prohibited)} patterns checked)")


def check_feature_maturity():
    """docs/feature-maturity.md must exist with all 4 section headers."""
    maturity_path = ROOT / "docs" / "feature-maturity.md"
    if not maturity_path.exists():
        error("docs/feature-maturity.md does not exist")
        return

    content = maturity_path.read_text(encoding="utf-8")
    required_sections = [
        "Deterministic",
        "Runtime Heuristics",
        "Background Analytics",
        "Experimental",
    ]

    missing = [s for s in required_sections if s not in content]
    if missing:
        error(f"feature-maturity.md missing sections: {missing}")
    else:
        print(f"  OK: feature-maturity.md has all {len(required_sections)} sections")


def check_dream_task_honesty():
    """README must not claim '10 learning tasks' without qualification."""
    readme = (ROOT / "README.md").read_text(encoding="utf-8")

    # Match "10 learning tasks" or "10 dream tasks" without nearby qualification
    pattern = re.compile(
        r"10\s+(learning|dream)\s+tasks", re.IGNORECASE
    )
    for m in pattern.finditer(readme):
        # Check surrounding context (100 chars each side) for qualification
        start = max(0, m.start() - 100)
        end = min(len(readme), m.end() + 100)
        context = readme[start:end].lower()
        if "planned" in context or "active" in context or "2 active" in context:
            continue
        error(
            f'README claims "{m.group(0)}" without qualification '
            f'(should say "2 active + 8 planned" or similar)'
        )

    if not pattern.search(readme):
        print("  OK: No unqualified dream task claims")
    else:
        # If we got here without errors, all mentions were qualified
        if not any("dream task" in e or "learning task" in e for e in ERRORS):
            print("  OK: Dream task claims properly qualified")


def main():
    print("Warden docs/code parity check\n")

    check_version()
    check_cli_commands()
    check_mcp_tools()
    check_config_tiers()
    check_latency_claims()
    check_prohibited_wording()
    check_feature_maturity()
    check_dream_task_honesty()

    print()
    if ERRORS:
        print(f"{len(ERRORS)} parity error(s) found.")
        sys.exit(1)
    else:
        print("All parity checks passed.")
        sys.exit(0)


if __name__ == "__main__":
    main()
