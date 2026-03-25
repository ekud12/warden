// ─── config::restrictions — restriction registry ─────────────────────────────
//
// Static registry of all restrictions with metadata. Used by the
// `restrictions` subcommand to display a formatted table, and by handlers
// to look up restriction properties (category, severity, disable-ability).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Safety,
    Destructive,
    Substitution,
    Governance,
    Hallucination,
    ZeroTrace,
    Redirect,
    Permission,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Category::Safety => write!(f, "Safety"),
            Category::Destructive => write!(f, "Destructive"),
            Category::Substitution => write!(f, "Substitution"),
            Category::Governance => write!(f, "Governance"),
            Category::Hallucination => write!(f, "Hallucination"),
            Category::ZeroTrace => write!(f, "ZeroTrace"),
            Category::Redirect => write!(f, "Redirect"),
            Category::Permission => write!(f, "Permission"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    HardDeny,
    SoftDeny,
    Advisory,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::HardDeny => write!(f, "HardDeny"),
            Severity::SoftDeny => write!(f, "SoftDeny"),
            Severity::Advisory => write!(f, "Advisory"),
        }
    }
}

pub struct Restriction {
    pub id: &'static str,
    pub handler: &'static str,
    pub category: Category,
    pub severity: Severity,
    pub description: &'static str,
    pub can_disable: bool,
}

pub static RESTRICTIONS: &[Restriction] = &[
    // ── Safety (8) ──────────────────────────────────────────────────────────
    Restriction {
        id: "safety.rm-rf",
        handler: "pretool-bash",
        category: Category::Safety,
        severity: Severity::HardDeny,
        description: "rm -rf or recursive deletion",
        can_disable: false,
    },
    Restriction {
        id: "safety.sudo",
        handler: "pretool-bash",
        category: Category::Safety,
        severity: Severity::HardDeny,
        description: "sudo/su/doas privilege escalation",
        can_disable: false,
    },
    Restriction {
        id: "safety.git-mutate",
        handler: "pretool-bash",
        category: Category::Safety,
        severity: Severity::SoftDeny,
        description: "Git mutating commands (OFF by default, enable: git_readonly = true)",
        can_disable: true,
    },
    Restriction {
        id: "safety.git-force",
        handler: "pretool-bash",
        category: Category::Safety,
        severity: Severity::SoftDeny,
        description: "Git force operations (OFF by default, enable: git_readonly = true)",
        can_disable: true,
    },
    Restriction {
        id: "safety.kill-9",
        handler: "pretool-bash",
        category: Category::Safety,
        severity: Severity::HardDeny,
        description: "Unguarded kill -9 / kill -KILL",
        can_disable: false,
    },
    Restriction {
        id: "safety.format-disk",
        handler: "pretool-bash",
        category: Category::Safety,
        severity: Severity::HardDeny,
        description: "Disk formatting commands",
        can_disable: false,
    },
    Restriction {
        id: "safety.registry-edit",
        handler: "pretool-bash",
        category: Category::Safety,
        severity: Severity::HardDeny,
        description: "Windows registry editing",
        can_disable: false,
    },
    Restriction {
        id: "safety.firewall",
        handler: "pretool-bash",
        category: Category::Safety,
        severity: Severity::HardDeny,
        description: "Firewall rule modification",
        can_disable: false,
    },
    // ── Destructive (3) ─────────────────────────────────────────────────────
    Restriction {
        id: "destructive.knip-fix",
        handler: "pretool-bash",
        category: Category::Destructive,
        severity: Severity::HardDeny,
        description: "knip --fix (auto-delete unused exports)",
        can_disable: true,
    },
    Restriction {
        id: "destructive.sg-rewrite",
        handler: "pretool-bash",
        category: Category::Destructive,
        severity: Severity::HardDeny,
        description: "sg -r (AST rewrite mode)",
        can_disable: true,
    },
    Restriction {
        id: "destructive.madge-image",
        handler: "pretool-bash",
        category: Category::Destructive,
        severity: Severity::HardDeny,
        description: "madge --image (overwrites dep graph)",
        can_disable: true,
    },
    // ── Substitution (8) ────────────────────────────────────────────────────
    Restriction {
        id: "substitution.grep",
        handler: "pretool-bash",
        category: Category::Substitution,
        severity: Severity::HardDeny,
        description: "grep \u{2192} rg",
        can_disable: true,
    },
    Restriction {
        id: "substitution.find",
        handler: "pretool-bash",
        category: Category::Substitution,
        severity: Severity::HardDeny,
        description: "find \u{2192} fd",
        can_disable: true,
    },
    Restriction {
        id: "substitution.curl",
        handler: "pretool-bash",
        category: Category::Substitution,
        severity: Severity::HardDeny,
        description: "curl \u{2192} xh",
        can_disable: true,
    },
    Restriction {
        id: "substitution.cat",
        handler: "pretool-bash",
        category: Category::Substitution,
        severity: Severity::HardDeny,
        description: "cat \u{2192} bat",
        can_disable: true,
    },
    Restriction {
        id: "substitution.ts-node",
        handler: "pretool-bash",
        category: Category::Substitution,
        severity: Severity::HardDeny,
        description: "ts-node \u{2192} tsx",
        can_disable: true,
    },
    Restriction {
        id: "substitution.ls",
        handler: "pretool-bash",
        category: Category::Substitution,
        severity: Severity::HardDeny,
        description: "ls \u{2192} eza",
        can_disable: true,
    },
    Restriction {
        id: "substitution.sd",
        handler: "pretool-bash",
        category: Category::Substitution,
        severity: Severity::HardDeny,
        description: "sd blocked on Windows",
        can_disable: true,
    },
    Restriction {
        id: "substitution.du",
        handler: "pretool-bash",
        category: Category::Substitution,
        severity: Severity::HardDeny,
        description: "du \u{2192} dust",
        can_disable: true,
    },
    // ── Hallucination HardDeny (11) ─────────────────────────────────────────
    Restriction {
        id: "hallucination.reverse-shell",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "Reverse shell patterns",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.credential-pipe",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "Credential piping to external services",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.ssh-write",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "SSH key/config writes",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.env-exfil",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "Environment variable exfiltration",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.base64-pipe",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "Encoded data piping to network",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.curl-post-secret",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "POSTing secrets/tokens to URLs",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.npm-publish",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "npm publish (unauthorized package release)",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.pip-install-url",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "pip install from arbitrary URLs",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.chmod-suid",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "SUID bit setting",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.crontab-write",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "Crontab modification",
        can_disable: false,
    },
    Restriction {
        id: "hallucination.hosts-edit",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::HardDeny,
        description: "/etc/hosts modification",
        can_disable: false,
    },
    // ── Hallucination Advisory (5) ──────────────────────────────────────────
    Restriction {
        id: "hallucination.deep-traverse",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::Advisory,
        description: "Deep directory traversal",
        can_disable: true,
    },
    Restriction {
        id: "hallucination.shell-config-write",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::Advisory,
        description: "Shell config file writes (.bashrc, .zshrc)",
        can_disable: true,
    },
    Restriction {
        id: "hallucination.global-install",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::Advisory,
        description: "Global package installation",
        can_disable: true,
    },
    Restriction {
        id: "hallucination.docker-prune",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::Advisory,
        description: "Docker system prune",
        can_disable: true,
    },
    Restriction {
        id: "hallucination.service-restart",
        handler: "pretool-bash",
        category: Category::Hallucination,
        severity: Severity::Advisory,
        description: "System service restart",
        can_disable: true,
    },
    // ── Read Governance (4) ─────────────────────────────────────────────────
    Restriction {
        id: "read.post-edit",
        handler: "pretool-read",
        category: Category::Governance,
        severity: Severity::Advisory,
        description: "Post-edit read advisory (file edited in last 2 turns)",
        can_disable: true,
    },
    Restriction {
        id: "read.dedup",
        handler: "pretool-read",
        category: Category::Governance,
        severity: Severity::Advisory,
        description: "Duplicate read advisory (same content hash)",
        can_disable: true,
    },
    Restriction {
        id: "read.large-file",
        handler: "pretool-read",
        category: Category::Governance,
        severity: Severity::SoftDeny,
        description: "Large file denial (>50KB code files)",
        can_disable: true,
    },
    Restriction {
        id: "read.progressive",
        handler: "pretool-read",
        category: Category::Governance,
        severity: Severity::SoftDeny,
        description: "Progressive read tightening (turn-based)",
        can_disable: true,
    },
    // ── Write Governance (2) ────────────────────────────────────────────────
    Restriction {
        id: "write.sensitive-path",
        handler: "pretool-write",
        category: Category::Governance,
        severity: Severity::HardDeny,
        description: "Sensitive path denial (.env, .ssh, etc.)",
        can_disable: false,
    },
    Restriction {
        id: "write.zero-trace",
        handler: "pretool-write",
        category: Category::ZeroTrace,
        severity: Severity::HardDeny,
        description: "Zero-trace content enforcement",
        can_disable: false,
    },
    // ── Redirect (3) ────────────────────────────────────────────────────────
    Restriction {
        id: "redirect.grep-tool",
        handler: "pretool-redirect",
        category: Category::Redirect,
        severity: Severity::HardDeny,
        description: "Grep tool \u{2192} rg via Bash",
        can_disable: true,
    },
    Restriction {
        id: "redirect.glob-tool",
        handler: "pretool-redirect",
        category: Category::Redirect,
        severity: Severity::HardDeny,
        description: "Glob tool \u{2192} fd via Bash",
        can_disable: true,
    },
    Restriction {
        id: "redirect.aidex-ext",
        handler: "pretool-redirect",
        category: Category::Redirect,
        severity: Severity::HardDeny,
        description: "aidex_signature unsupported extension guard",
        can_disable: true,
    },
    // ── Permission (4) ──────────────────────────────────────────────────────
    Restriction {
        id: "permission.env-files",
        handler: "permission-approve",
        category: Category::Permission,
        severity: Severity::HardDeny,
        description: ".env file access",
        can_disable: false,
    },
    Restriction {
        id: "permission.git-dir",
        handler: "permission-approve",
        category: Category::Permission,
        severity: Severity::HardDeny,
        description: ".git directory access",
        can_disable: false,
    },
    Restriction {
        id: "permission.node-modules",
        handler: "permission-approve",
        category: Category::Permission,
        severity: Severity::HardDeny,
        description: "node_modules directory access",
        can_disable: false,
    },
    Restriction {
        id: "permission.credentials",
        handler: "permission-approve",
        category: Category::Permission,
        severity: Severity::HardDeny,
        description: "Credential/secret file access",
        can_disable: false,
    },
];

/// CLI entry point: print restriction registry as a formatted table.
/// Supports `--category <name>` to filter by category (case-insensitive).
pub fn run(args: &[String]) {
    let filter = parse_category_filter(args);

    let entries: Vec<&Restriction> = RESTRICTIONS
        .iter()
        .filter(|r| match &filter {
            Some(cat) => r.category == *cat,
            None => true,
        })
        .collect();

    if entries.is_empty() {
        eprintln!("No restrictions found.");
        return;
    }

    // Column widths
    let w_id = entries.iter().map(|r| r.id.len()).max().unwrap_or(2).max(2);
    let w_hand = entries
        .iter()
        .map(|r| r.handler.len())
        .max()
        .unwrap_or(7)
        .max(7);
    let w_cat = entries
        .iter()
        .map(|r| format!("{}", r.category).len())
        .max()
        .unwrap_or(8)
        .max(8);
    let w_sev = entries
        .iter()
        .map(|r| format!("{}", r.severity).len())
        .max()
        .unwrap_or(8)
        .max(8);
    let w_dis = 11; // "Can Disable"
    let w_desc = entries
        .iter()
        .map(|r| r.description.len())
        .max()
        .unwrap_or(11)
        .max(11);

    // Header
    println!(
        "{:<w_id$} | {:<w_hand$} | {:<w_cat$} | {:<w_sev$} | {:<w_dis$} | {:<w_desc$}",
        "ID",
        "Handler",
        "Category",
        "Severity",
        "Can Disable",
        "Description",
        w_id = w_id,
        w_hand = w_hand,
        w_cat = w_cat,
        w_sev = w_sev,
        w_dis = w_dis,
        w_desc = w_desc,
    );
    println!(
        "{}-+-{}-+-{}-+-{}-+-{}-+-{}",
        "-".repeat(w_id),
        "-".repeat(w_hand),
        "-".repeat(w_cat),
        "-".repeat(w_sev),
        "-".repeat(w_dis),
        "-".repeat(w_desc),
    );

    // Rows
    for r in &entries {
        let dis = if r.can_disable { "yes" } else { "no" };
        println!(
            "{:<w_id$} | {:<w_hand$} | {:<w_cat$} | {:<w_sev$} | {:<w_dis$} | {}",
            r.id,
            r.handler,
            r.category,
            r.severity,
            dis,
            r.description,
            w_id = w_id,
            w_hand = w_hand,
            w_cat = w_cat,
            w_sev = w_sev,
            w_dis = w_dis,
        );
    }

    println!("\nTotal: {} restrictions", entries.len());
}

/// Parse `--category <name>` from args (case-insensitive).
fn parse_category_filter(args: &[String]) -> Option<Category> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--category"
            && let Some(val) = args.get(i + 1)
        {
            return match val.to_lowercase().as_str() {
                "safety" => Some(Category::Safety),
                "destructive" => Some(Category::Destructive),
                "substitution" => Some(Category::Substitution),
                "governance" => Some(Category::Governance),
                "hallucination" => Some(Category::Hallucination),
                "zerotrace" | "zero-trace" | "zero_trace" => Some(Category::ZeroTrace),
                "redirect" => Some(Category::Redirect),
                "permission" => Some(Category::Permission),
                other => {
                    eprintln!(
                        "Unknown category: {}. Valid: Safety, Destructive, Substitution, Governance, Hallucination, ZeroTrace, Redirect, Permission",
                        other
                    );
                    std::process::exit(1);
                }
            };
        }
        i += 1;
    }
    None
}
