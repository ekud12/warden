/// Version consistency tests — ensure Cargo.toml, CHANGELOG.md, and binary agree.

#[test]
fn cargo_toml_version_is_current() {
    let cargo_toml = include_str!("../Cargo.toml");
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!("version = \"{}\"", version);
    assert!(
        cargo_toml.contains(&expected),
        "Cargo.toml version field doesn't match CARGO_PKG_VERSION: {}",
        version
    );
}

#[test]
fn changelog_has_current_version() {
    let changelog = include_str!("../CHANGELOG.md");
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!("[{}]", version);
    assert!(
        changelog.contains(&expected),
        "CHANGELOG.md missing section for version {}",
        version
    );
}

#[test]
fn changelog_current_version_is_first() {
    let changelog = include_str!("../CHANGELOG.md");
    let version = env!("CARGO_PKG_VERSION");
    let expected = format!("## [{}]", version);
    // Find the first ## section — it should be the current version
    let first_section = changelog
        .lines()
        .find(|line| line.starts_with("## ["))
        .expect("CHANGELOG.md has no version sections");
    assert!(
        first_section.contains(&format!("[{}]", version)),
        "First CHANGELOG.md section is '{}', expected version {}",
        first_section,
        version
    );
}
