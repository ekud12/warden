// ─── common::util — shared utility functions ─────────────────────────────────

use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};

/// Truncate a string to max length, appending "..." if truncated
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

/// Compute a fast content hash for a file: hash(size + first 512 bytes + last 512 bytes)
/// Returns None if file can't be read/stat'd
pub fn content_hash(path: &std::path::Path) -> Option<u64> {
    let meta = fs::metadata(path).ok()?;
    let size = meta.len();

    let mut f = fs::File::open(path).ok()?;
    let mut head = vec![0u8; 512.min(size as usize)];
    let head_read = Read::read(&mut f, &mut head).ok()?;
    head.truncate(head_read);

    let mut tail = Vec::new();
    if size > 512 {
        let tail_start = size.saturating_sub(512);
        let _ = f.seek(SeekFrom::Start(tail_start));
        let mut buf = vec![0u8; 512.min((size - tail_start) as usize)];
        let tail_read = Read::read(&mut f, &mut buf).ok()?;
        buf.truncate(tail_read);
        tail = buf;
    }

    let mut hasher = std::hash::DefaultHasher::new();
    size.hash(&mut hasher);
    head.hash(&mut hasher);
    tail.hash(&mut hasher);
    Some(hasher.finish())
}

/// Hash a string slice (for command output dedup)
pub fn string_hash(s: &str) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// ISO 8601 timestamp without external crate
pub fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    // Convert epoch seconds to Y-M-D H:M:S (UTC)
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, m, s)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    loop {
        let ydays = if is_leap(y) { 366 } else { 365 };
        if days < ydays {
            break;
        }
        days -= ydays;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0usize;
    while mo < 12 && days >= mdays[mo] {
        days -= mdays[mo];
        mo += 1;
    }
    (y, (mo + 1) as u64, days + 1)
}

fn is_leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}

/// Get file modification time as seconds since epoch (for stale-read detection)
pub fn file_mtime(path: &std::path::Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Check if string contains suspicious control characters or invisible unicode.
/// Returns description of what was found, or None if clean.
pub fn detect_suspicious_chars(s: &str) -> Option<String> {
    let mut found = Vec::new();

    for (i, b) in s.bytes().enumerate() {
        // Control chars except tab(0x09), newline(0x0A), carriage return(0x0D)
        if b <= 0x08 || b == 0x0B || b == 0x0C || (0x0E..=0x1F).contains(&b) {
            found.push(format!("control char 0x{:02x} at byte {}", b, i));
            if found.len() >= 3 {
                break;
            }
        }
    }

    for c in s.chars() {
        match c {
            '\u{200B}'..='\u{200F}' | '\u{FEFF}' => {
                found.push(format!("zero-width char U+{:04X}", c as u32));
            }
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => {
                found.push(format!("directional override U+{:04X}", c as u32));
            }
            '\u{E0001}'..='\u{E007F}' => {
                found.push(format!("tag char U+{:04X}", c as u32));
            }
            _ => {}
        }
        if found.len() >= 3 {
            break;
        }
    }

    if found.is_empty() {
        None
    } else {
        Some(found.join(", "))
    }
}

/// Normalize path separators for comparison (backslash → forward slash)
pub fn normalize_path(p: &str) -> String {
    p.replace('\\', "/")
}

/// Strip ANSI escape sequences from a string (manual state machine, no regex)
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;
    for c in s.chars() {
        if in_escape {
            if c.is_ascii_alphabetic() || c == 'm' || c == 'K' || c == 'H' || c == 'J' {
                in_escape = false;
            }
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            out.push(c);
        }
    }
    out
}
