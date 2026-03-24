// ─── core::sensitive_paths — file write protection patterns ──────────────────

/// Sensitive paths: DENY writes (credential directories, system paths)
pub const SENSITIVE_PATHS_DENY: &[(&str, &str)] = &[
    (r"[\\/]\.ssh[\\/]", "BLOCKED: Cannot write to .ssh directory."),
    (r"[\\/]\.gnupg[\\/]", "BLOCKED: Cannot write to .gnupg directory."),
    (r"(?i)([\\/]|^)\.(git-credentials|netrc)$", "BLOCKED: Cannot write to credential files."),
    (r"(?i)^(/etc/|/usr/|/var/|C:[/\\]Windows|C:[/\\]Program Files)", "BLOCKED: Cannot write to system directories."),
    (r"(?i)([\\/]|^)\.aws[\\/]credentials$", "BLOCKED: Cannot write to AWS credentials."),
    (r"(?i)([\\/]|^)\.azure[\\/]", "BLOCKED: Cannot write to Azure config directory."),
    (r"(?i)([\\/]|^)\.kube[\\/]config$", "BLOCKED: Cannot write to Kubernetes config."),
    (r"(?i)([\\/]|^)\.docker[\\/]config\.json$", "BLOCKED: Cannot write to Docker credentials."),
    (r"(?i)([\\/]|^)\.gcloud[\\/]", "BLOCKED: Cannot write to GCloud config directory."),
    (r"(?i)([\\/]|^)\.(pem|key|p12|pfx)$", "BLOCKED: Cannot write certificate/key files."),
    (r"(?i)([\\/]|^)id_rsa", "BLOCKED: Cannot write SSH private keys."),
    (r"(?i)([\\/]|^)id_ed25519", "BLOCKED: Cannot write SSH private keys."),
    (r"(?i)([\\/]|^)\.terraform[\\/]", "BLOCKED: Cannot write to Terraform state directory."),
    (r"(?i)([\\/]|^)\.vault-token$", "BLOCKED: Cannot write HashiCorp Vault token."),
    (r"(?i)\.(keystore|jks|p12)$", "BLOCKED: Cannot write Java/Android keystore files."),
];

/// Sensitive paths: ADVISORY (suspicious but potentially legitimate)
pub const SENSITIVE_PATHS_WARN: &[(&str, &str)] = &[
    (r"[\\/]\.git[\\/]hooks[\\/]", "Advisory: Writing to .git/hooks/. Verify intentional."),
    (r"(?i)([\\/]|^)\.(bashrc|zshrc|profile|bash_profile)$", "Advisory: Writing to shell config file."),
    (r"(?i)([\\/]|^)\.npmrc$", "Advisory: Writing to .npmrc."),
    (r"(?i)([\\/]|^)\.env(\.|$)", "Advisory: Writing to .env file. Ensure no secrets are hardcoded."),
    (r"(?i)([\\/]|^)docker-compose.*\.ya?ml$", "Advisory: Modifying Docker Compose config."),
    (r"(?i)([\\/]|^)\.github[\\/]workflows[\\/]", "Advisory: Modifying CI/CD pipeline."),
    (r"(?i)([\\/]|^)\.gitlab-ci\.yml$", "Advisory: Modifying GitLab CI pipeline."),
    (r"(?i)([\\/]|^)Dockerfile$", "Advisory: Modifying Dockerfile. Verify build context."),
    (r"(?i)([\\/]|^)Makefile$", "Advisory: Modifying Makefile. Review targets carefully."),
    (r"(?i)([\\/]|^)secrets\.ya?ml$", "Advisory: Modifying secrets manifest. Ensure no plaintext secrets."),
    (r"(?i)([\\/]|^)\.circleci[\\/]config\.yml$", "Advisory: Modifying CircleCI pipeline."),
    (r"(?i)([\\/]|^)Jenkinsfile$", "Advisory: Modifying Jenkins pipeline."),
];
