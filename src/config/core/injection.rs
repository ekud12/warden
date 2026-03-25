// ─── core::injection — prompt injection detection patterns ───────────────────

/// Injection: (regex, category) — matched against tool output to detect attacks
pub const INJECTION_PATTERNS: &[(&str, &str)] = &[
    // Instruction hijacking
    (
        r"(?i)ignore\s+(all\s+)?previous\s+instructions",
        "instruction-hijack",
    ),
    (
        r"(?i)forget\s+(all\s+)?(your\s+)?instructions",
        "instruction-hijack",
    ),
    (
        r"(?i)disregard\s+(all\s+)?(previous|above|prior)\s+(instructions|rules|context)",
        "instruction-hijack",
    ),
    (
        r"(?i)override\s+(all\s+)?(previous|prior|your)\s+(instructions|rules)",
        "instruction-hijack",
    ),
    (r"(?i)new\s+instructions?\s*:", "instruction-hijack"),
    (r"(?i)system\s+prompt\s*:", "instruction-hijack"),
    (r"(?i)<\s*system\s*>", "instruction-hijack"),
    (
        r"(?i)do\s+not\s+follow\s+(the\s+)?(previous|original|user)",
        "instruction-hijack",
    ),
    (
        r"(?i)the\s+real\s+instructions\s+(are|follow)",
        "instruction-hijack",
    ),
    (
        r"(?i)actually,?\s+ignore\s+(that|this|everything)",
        "instruction-hijack",
    ),
    // Role manipulation
    (r"(?i)you\s+are\s+now\s+(a|an|my)\s+", "role-manipulation"),
    (
        r"(?i)your\s+new\s+(role|task|instruction|purpose)",
        "role-manipulation",
    ),
    (
        r"(?i)from\s+now\s+on\s+you\s+(will|must|should)",
        "role-manipulation",
    ),
    (r"(?i)switch\s+to\s+.{0,20}\s+mode", "role-manipulation"),
    (r"(?i)pretend\s+(you\s+are|to\s+be)\s+", "role-manipulation"),
    (
        r"(?i)act\s+as\s+(if|though)\s+you\s+(are|were)\s+",
        "role-manipulation",
    ),
    (r"(?i)simulate\s+(being|a)\s+", "role-manipulation"),
    // Data exfiltration
    (
        r"(?i)(send|forward|transmit)\s+(this|the|all|every)\s+(to|data)\s+",
        "exfiltration",
    ),
    (r"(?i)exfiltrate", "exfiltration"),
    (
        r"(?i)(upload|post|share)\s+(this|the|all)\s+(data|content|code|file)",
        "exfiltration",
    ),
    (
        r"(?i)encode\s+(the\s+)?(content|data|file)\s+as\s+base64",
        "exfiltration",
    ),
    // Tool manipulation
    (
        r"(?i)execute\s+the\s+following\s+(command|code|script)",
        "tool-manipulation",
    ),
    (
        r"(?i)run\s+this\s+(bash|shell|command|script)\s*:",
        "tool-manipulation",
    ),
    (r"(?i)delete\s+(all|every)\s+files?\b", "tool-manipulation"),
    (r"(?i)write\s+this\s+(to|into)\s+/", "tool-manipulation"),
    (
        r"(?i)create\s+a\s+file\s+(at|in)\s+/etc/",
        "tool-manipulation",
    ),
    (
        r"(?i)modify\s+the\s+(system|config|settings)\s+to\s+",
        "tool-manipulation",
    ),
    // Prompt extraction
    (
        r"(?i)(show|reveal|display|print)\s+(your|the)\s+(system\s+)?prompt",
        "prompt-extraction",
    ),
    (
        r"(?i)what\s+(are|is)\s+your\s+(instructions|system\s+prompt|rules)",
        "prompt-extraction",
    ),
    (
        r"(?i)output\s+your\s+(full\s+)?(system\s+)?prompt",
        "prompt-extraction",
    ),
    (
        r"(?i)repeat\s+(back\s+)?(your|the)\s+entire\s+(system\s+)?prompt",
        "prompt-extraction",
    ),
    (
        r"(?i)dump\s+(your|the)\s+(instructions|context|system\s+message)",
        "prompt-extraction",
    ),
    // Social engineering
    (
        r"(?i)the\s+user\s+(wants|asked|said)\s+you\s+to\s+",
        "social-engineering",
    ),
    (
        r"(?i)this\s+is\s+an?\s+(emergency|urgent|critical)\s+.*\s+(override|bypass)",
        "social-engineering",
    ),
    (
        r"(?i)administrator\s+(here|speaking|override)",
        "social-engineering",
    ),
    // Script/markup injection in tool output
    (r"<script[^>]*>", "script-injection"),
    (r"javascript:", "script-injection"),
    (r"on(error|load|click|mouseover)\s*=", "script-injection"),
];
