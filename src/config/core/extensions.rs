// ─── config::core::extensions — file extension constants ──────────────────────

/// Code file extensions for Read governance
pub const CODE_EXTS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "cs", "py", "go", "rs", "java", "vue", "svelte", "rb", "kt", "swift",
    "cpp", "c", "h",
];

/// File extensions that aidex can parse signatures for (subset of CODE_EXTS)
pub const AIDEX_EXTS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "cs", "py", "go", "rs", "java", "rb",
];

/// Code extensions regex for edit tracking
pub const CODE_EXTS_REGEX: &str = r"\.(ts|tsx|js|jsx|cs|py|go|rs|java|vue|svelte|astro|css)$";
