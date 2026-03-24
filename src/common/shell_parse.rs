// ─── shell_parse — lightweight shell command tokenizer ────────────────────────
//
// Quote-aware splitting of compound shell commands into segments.
// Handles: &&, ||, ;, | as separators while respecting single/double quotes,
// backslash escapes, and $(...) subshells.
//
// NOT a full bash parser — designed for the 99% case of commands Claude generates.
// ──────────────────────────────────────────────────────────────────────────────

/// A single command segment from a compound command string.
#[derive(Debug, Clone)]
pub struct Segment {
    /// The command text (trimmed)
    pub text: String,
    /// Separator that follows this segment ("&&", "||", ";", "|", or "" for last)
    pub separator: String,
}

impl Segment {
    /// Extract the base command name (first word, ignoring env vars and leading whitespace).
    /// Examples: "cargo build" → "cargo", "FOO=1 bar" → "bar", "  ls -la" → "ls"
    #[cfg(test)]
    pub fn base_command(&self) -> &str {
        let s = self.text.trim();
        // Skip env var assignments (KEY=VALUE prefix)
        let mut rest = s;
        while let Some(eq_pos) = rest.find('=') {
            let before_eq = &rest[..eq_pos];
            // Valid env var: all uppercase/lowercase/digits/underscore, no spaces
            if !before_eq.is_empty()
                && before_eq.chars().all(|c| c.is_alphanumeric() || c == '_')
                && before_eq.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
            {
                // Skip past the value
                let after_eq = &rest[eq_pos + 1..];
                // Value might be quoted
                if after_eq.starts_with('"') || after_eq.starts_with('\'') {
                    let quote = after_eq.as_bytes()[0] as char;
                    if let Some(end) = after_eq[1..].find(quote) {
                        rest = after_eq[end + 2..].trim_start();
                        continue;
                    }
                }
                // Unquoted value — skip to next space
                rest = after_eq
                    .find(' ')
                    .map(|i| after_eq[i..].trim_start())
                    .unwrap_or("");
                continue;
            }
            break;
        }
        // First word of remaining
        rest.split_whitespace().next().unwrap_or("")
    }
}

/// Parse a compound shell command into segments, respecting quotes and escapes.
pub fn parse(cmd: &str) -> Vec<Segment> {
    let chars: Vec<char> = cmd.chars().collect();
    let len = chars.len();
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut i = 0;

    // Quote/escape state
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut paren_depth: u32 = 0; // Track $(...) depth

    while i < len {
        let ch = chars[i];

        // Backslash escape (outside single quotes)
        if ch == '\\' && !in_single_quote && i + 1 < len {
            current.push(ch);
            current.push(chars[i + 1]);
            i += 2;
            continue;
        }

        // Single quote toggle
        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            current.push(ch);
            i += 1;
            continue;
        }

        // Double quote toggle
        if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            current.push(ch);
            i += 1;
            continue;
        }

        // Inside quotes — everything is literal
        if in_single_quote || in_double_quote {
            current.push(ch);
            i += 1;
            continue;
        }

        // Track $(...) subshell depth
        if ch == '$' && i + 1 < len && chars[i + 1] == '(' {
            paren_depth += 1;
            current.push(ch);
            current.push('(');
            i += 2;
            continue;
        }
        if ch == '(' && paren_depth > 0 {
            paren_depth += 1;
            current.push(ch);
            i += 1;
            continue;
        }
        if ch == ')' && paren_depth > 0 {
            paren_depth -= 1;
            current.push(ch);
            i += 1;
            continue;
        }

        // Inside subshell — don't split
        if paren_depth > 0 {
            current.push(ch);
            i += 1;
            continue;
        }

        // Check for separators (outside quotes and subshells)
        // && operator
        if ch == '&' && i + 1 < len && chars[i + 1] == '&' {
            push_segment(&mut segments, &mut current, "&&");
            i += 2;
            skip_whitespace(&chars, &mut i);
            continue;
        }

        // || operator
        if ch == '|' && i + 1 < len && chars[i + 1] == '|' {
            push_segment(&mut segments, &mut current, "||");
            i += 2;
            skip_whitespace(&chars, &mut i);
            continue;
        }

        // ; separator
        if ch == ';' {
            push_segment(&mut segments, &mut current, ";");
            i += 1;
            skip_whitespace(&chars, &mut i);
            continue;
        }

        // | pipe (single)
        if ch == '|' {
            push_segment(&mut segments, &mut current, "|");
            i += 1;
            skip_whitespace(&chars, &mut i);
            continue;
        }

        current.push(ch);
        i += 1;
    }

    // Final segment
    let text = current.trim().to_string();
    if !text.is_empty() {
        segments.push(Segment {
            text,
            separator: String::new(),
        });
    }

    segments
}

fn push_segment(segments: &mut Vec<Segment>, current: &mut String, sep: &str) {
    let text = current.trim().to_string();
    if !text.is_empty() {
        segments.push(Segment {
            text,
            separator: sep.to_string(),
        });
    }
    current.clear();
}

fn skip_whitespace(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i] == ' ' {
        *i += 1;
    }
}

/// Rejoin segments back into a command string with separators.
pub fn rejoin(segments: &[Segment]) -> String {
    let mut result = String::new();
    for (idx, seg) in segments.iter().enumerate() {
        if idx > 0 {
            result.push(' ');
        }
        result.push_str(&seg.text);
        if !seg.separator.is_empty() {
            result.push(' ');
            result.push_str(&seg.separator);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_command() {
        let segs = parse("ls -la");
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "ls -la");
        assert_eq!(segs[0].base_command(), "ls");
    }

    #[test]
    fn compound_and() {
        let segs = parse("cd /tmp && ls -la");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "cd /tmp");
        assert_eq!(segs[0].separator, "&&");
        assert_eq!(segs[1].text, "ls -la");
        assert_eq!(segs[1].base_command(), "ls");
    }

    #[test]
    fn quoted_separators() {
        let segs = parse(r#"echo "hello && world" && ls"#);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, r#"echo "hello && world""#);
        assert_eq!(segs[1].text, "ls");
    }

    #[test]
    fn single_quoted() {
        let segs = parse("echo 'foo; bar' ; ls");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "echo 'foo; bar'");
        assert_eq!(segs[1].text, "ls");
    }

    #[test]
    fn pipe() {
        let segs = parse("cat file | rg pattern | wc -l");
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].base_command(), "cat");
        assert_eq!(segs[0].separator, "|");
        assert_eq!(segs[1].base_command(), "rg");
        assert_eq!(segs[2].base_command(), "wc");
    }

    #[test]
    fn subshell() {
        let segs = parse("echo $(cd /tmp && ls)");
        assert_eq!(segs.len(), 1); // $() is not split
        assert_eq!(segs[0].text, "echo $(cd /tmp && ls)");
    }

    #[test]
    fn env_var_prefix() {
        let seg = Segment {
            text: "FOO=bar cargo build".into(),
            separator: String::new(),
        };
        assert_eq!(seg.base_command(), "cargo");
    }

    #[test]
    fn or_operator() {
        let segs = parse("test -f foo || echo missing");
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].separator, "||");
        assert_eq!(segs[1].base_command(), "echo");
    }

    #[test]
    fn rejoin_roundtrip() {
        let input = "cd /tmp && ls -la ; echo done";
        let segs = parse(input);
        let output = rejoin(&segs);
        assert_eq!(output, "cd /tmp && ls -la ; echo done");
    }
}
