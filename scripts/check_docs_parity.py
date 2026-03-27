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
    """Cargo.toml version must match README badge."""
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.MULTILINE)
    if not m:
        error("Could not parse version from Cargo.toml")
        return
    cargo_ver = m.group(1)

    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    badge_match = re.search(r"badge/v([\d.]+)-blue", readme)
    if not badge_match:
        error("Could not find version badge in README.md")
        return
    readme_ver = badge_match.group(1)

    if cargo_ver != readme_ver:
        error(f"Version mismatch: Cargo.toml={cargo_ver}, README badge={readme_ver}")
    else:
        print(f"  OK: Version {cargo_ver}")


def check_cli_commands():
    """README command table entries should cover USER_COMMANDS."""
    cli_src = (ROOT / "src" / "cli" / "mod.rs").read_text(encoding="utf-8")

    # Extract USER_COMMANDS array
    m = re.search(
        r'pub const USER_COMMANDS.*?=.*?\[([^\]]+)\]', cli_src, re.DOTALL
    )
    if not m:
        error("Could not parse USER_COMMANDS from src/cli/mod.rs")
        return
    code_cmds = set(re.findall(r'"([^"]+)"', m.group(1)))

    # Extract commands from README table (lines starting with | `warden ...)
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    readme_cmds = set()
    for m in re.finditer(r"\|\s*`warden\s+(\S+)", readme):
        cmd = m.group(1).rstrip("`").split()[0]
        # Normalize: server-status, server-stop etc
        readme_cmds.add(cmd)

    # Commands in code but not in README
    missing = code_cmds - readme_cmds
    # Filter out internal-only commands
    internal = {"redb", "state"}
    missing -= internal

    if missing:
        error(f"CLI commands in code but not in README: {sorted(missing)}")
    else:
        print(f"  OK: {len(code_cmds)} CLI commands documented")


def check_mcp_tools():
    """README MCP tool names should match tools_list() in mcp.rs."""
    mcp_src = (ROOT / "src" / "engines" / "harbor" / "mcp.rs").read_text(
        encoding="utf-8"
    )

    # Extract tool names from tools_list function
    code_tools = set(re.findall(r'"name":\s*"(\w+)"', mcp_src))

    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    # Extract from MCP table: | `tool_name` |
    readme_tools = set(re.findall(r"\|\s*`(\w+)`\s*\|", readme))
    # Filter to only MCP-looking names
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


def main():
    print("Warden docs/code parity check\n")

    check_version()
    check_cli_commands()
    check_mcp_tools()
    check_config_tiers()
    check_latency_claims()

    print()
    if ERRORS:
        print(f"{len(ERRORS)} parity error(s) found.")
        sys.exit(1)
    else:
        print("All parity checks passed.")
        sys.exit(0)


if __name__ == "__main__":
    main()
