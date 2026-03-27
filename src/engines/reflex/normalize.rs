// ─── Normalize — Command normalization for evasion resistance ─────────────────
//
// Normalizes shell commands before pattern matching:
//   1. Whitespace collapse — "rm   -rf  /" → "rm -rf /"
//   2. Quote stripping — remove balanced quotes around arguments
//   3. Compound splitting — split on &&, ||, ; → check each part
//   4. Alias expansion — resolve common shell aliases
// ──────────────────────────────────────────────────────────────────────────────

/// Normalize a command. Returns each compound sub-command normalized.
pub fn normalize(cmd: &str) -> Vec<String> {
    split_compound(cmd)
        .into_iter()
        .map(|p| expand_alias(&strip_quotes(&collapse_whitespace(&p))))
        .collect()
}

fn collapse_whitespace(cmd: &str) -> String {
    let mut result = String::with_capacity(cmd.len());
    let mut last_ws = true;
    for ch in cmd.chars() {
        if ch.is_whitespace() {
            if !last_ws {
                result.push(' ');
                last_ws = true;
            }
        } else {
            result.push(ch);
            last_ws = false;
        }
    }
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

fn strip_quotes(cmd: &str) -> String {
    let mut result = String::with_capacity(cmd.len());
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if (ch == '"' || ch == '\'')
            && i + 1 < chars.len()
            && let Some(end) = find_closing_quote(&chars, i, ch)
        {
            for c in &chars[(i + 1)..end] {
                result.push(*c);
            }
            i = end + 1;
            continue;
        }
        result.push(ch);
        i += 1;
    }
    result
}

fn find_closing_quote(chars: &[char], start: usize, quote: char) -> Option<usize> {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            i += 2;
            continue;
        }
        if chars[i] == quote {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn split_compound(cmd: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    let (mut sq, mut dq) = (false, false);

    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' && !dq {
            sq = !sq;
            current.push(ch);
            i += 1;
            continue;
        }
        if ch == '"' && !sq {
            dq = !dq;
            current.push(ch);
            i += 1;
            continue;
        }
        if !sq && !dq {
            if i + 1 < chars.len()
                && ((ch == '&' && chars[i + 1] == '&') || (ch == '|' && chars[i + 1] == '|'))
            {
                let t = current.trim().to_string();
                if !t.is_empty() {
                    parts.push(t);
                }
                current.clear();
                i += 2;
                continue;
            }
            if ch == ';' {
                let t = current.trim().to_string();
                if !t.is_empty() {
                    parts.push(t);
                }
                current.clear();
                i += 1;
                continue;
            }
        }
        current.push(ch);
        i += 1;
    }
    let t = current.trim().to_string();
    if !t.is_empty() {
        parts.push(t);
    }
    if parts.is_empty() {
        parts.push(String::new());
    }
    parts
}

fn expand_alias(cmd: &str) -> String {
    let end = cmd.find(' ').unwrap_or(cmd.len());
    let (first, rest) = (&cmd[..end], &cmd[end..]);
    match first {
        "ll" => format!("ls -la{}", rest),
        "la" => format!("ls -a{}", rest),
        "l" => format!("ls -CF{}", rest),
        "md" => format!("mkdir -p{}", rest),
        "rd" => format!("rmdir{}", rest),
        _ => cmd.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_collapse() {
        assert_eq!(collapse_whitespace("rm   -rf  /"), "rm -rf /");
    }

    #[test]
    fn quote_strip() {
        assert_eq!(strip_quotes(r#""rm" "-rf" '/'"#), "rm -rf /");
    }

    #[test]
    fn compound_split() {
        let p = split_compound("echo hi && rm -rf / || echo done");
        assert_eq!(p, vec!["echo hi", "rm -rf /", "echo done"]);
    }

    #[test]
    fn compound_preserves_quotes() {
        let p = split_compound(r#"echo "a && b" && rm -rf /"#);
        assert_eq!(p, vec![r#"echo "a && b""#, "rm -rf /"]);
    }

    #[test]
    fn alias_expand() {
        assert_eq!(expand_alias("ll /tmp"), "ls -la /tmp");
        assert_eq!(expand_alias("cargo build"), "cargo build");
    }

    #[test]
    fn full_pipeline() {
        let parts = normalize("echo  hi  &&  'rm'  -rf  /");
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "echo hi");
        assert_eq!(parts[1], "rm -rf /");
    }

    #[test]
    fn evasion_compound_hidden() {
        let parts = normalize("echo safe && rm -rf /");
        assert!(parts[1].contains("rm -rf"));
    }
}
