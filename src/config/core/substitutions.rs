// ─── core::substitutions — CLI tool substitution patterns ────────────────────
//
// Two types:
//   TRANSFORMS: silently rewrite command (tool-name swap, compatible output)
//   DENIALS: block + suggest (incompatible output or dangerous)

/// Transform-eligible substitutions: (regex, source_tool, target_tool)
/// These silently rewrite the command — zero friction, same output.
pub const TRANSFORMS: &[(&str, &str, &str)] = &[
    (r"\bgrep\s", "grep", "rg"),
    (r"\bfind\s", "find", "fd"),
    (r"\bdu\s", "du", "dust"),
    (r"\bsort\b[^|]*\|\s*uniq\b|\bsort\s+-u\b", "sort", "huniq"),
];

/// Denial substitutions: (regex, deny_message) — block, don't transform.
/// These have incompatible output or are dangerous.
pub const SUBSTITUTIONS: &[(&str, &str)] = &[
    (
        r"\bcurl\s",
        "BLOCKED: Use xh instead of curl. Example: xh GET url.",
    ),
    (
        r"\bts-node\b",
        "BLOCKED: Use tsx instead of ts-node. Example: tsx script.ts",
    ),
    (
        r"\bsd\s",
        "BLOCKED: sd mangles newlines on Windows. Use the Edit tool for file modifications.",
    ),
    (
        r"\btar\s+(x|c|z)",
        "BLOCKED: Use ouch instead of tar. Example: ouch compress/decompress FILE.",
    ),
    (
        r"\bzip\s",
        "BLOCKED: Use ouch instead of zip. Example: ouch compress FILES OUTPUT.",
    ),
    (
        r"\bunzip\s",
        "BLOCKED: Use ouch instead of unzip. Example: ouch decompress FILE.",
    ),
    (
        r"\bgzip\s",
        "BLOCKED: Use ouch instead of gzip. Example: ouch compress/decompress FILE.",
    ),
];
