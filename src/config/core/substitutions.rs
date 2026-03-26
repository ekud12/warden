// ─── core::substitutions — CLI tool substitution patterns (DENY) ─────────────

/// Substitutions: (regex, deny_message) — redirect to modern CLI tools
/// Each substitution only fires if the target tool is installed (checked at runtime).
pub const SUBSTITUTIONS: &[(&str, &str)] = &[
    (
        r"\bgrep\s",
        "BLOCKED: Use rg (ripgrep) instead of grep. Same flags: rg PATTERN [PATH].",
    ),
    (
        r"\bfind\s",
        "BLOCKED: Use fd instead of find. Example: fd PATTERN [PATH]. Regex by default.",
    ),
    (
        r"\bcurl\s",
        "BLOCKED: Use xh instead of curl. Example: xh GET url.",
    ),
    (
        r"\bts-node\b",
        "BLOCKED: Use tsx instead of ts-node. Example: tsx script.ts",
    ),
    (
        r"\bdu\s",
        "BLOCKED: Use dust instead of du. Example: dust . or dust -d 2.",
    ),
    (
        r"\bsort\b[^|]*\|\s*uniq\b|\bsort\s+-u\b",
        "BLOCKED: Use huniq instead of sort|uniq. Faster, preserves order.",
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
    (
        r"\bcat\s+(?!<<)[^\|]+$",
        "BLOCKED: Use bat instead of cat. Example: bat FILE (syntax highlighting).",
    ),
];
