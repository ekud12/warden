// ─── install::term — interactive terminal components ──────────────────────────
//
// Arrow-key selection, multi-select checkboxes, styled output.
// Uses crossterm directly (no ratatui) for inline terminal UI.
// ──────────────────────────────────────────────────────────────────────────────

use crossterm::{
    ExecutableCommand, cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::io::{self, Write};

// ─── Colors ──────────────────────────────────────────────────────────────────

pub const BRAND: Color = Color::Rgb {
    r: 220,
    g: 38,
    b: 38,
}; // Warden red (logo)
pub const ACCENT: Color = Color::Rgb {
    r: 153,
    g: 27,
    b: 27,
}; // Dark crimson
pub const SUCCESS: Color = Color::Rgb {
    r: 34,
    g: 197,
    b: 94,
}; // Green
pub const WARN: Color = Color::Rgb {
    r: 234,
    g: 179,
    b: 8,
}; // Yellow
pub const ERROR: Color = Color::Rgb {
    r: 239,
    g: 68,
    b: 68,
}; // Red
pub const DIM: Color = Color::Rgb {
    r: 107,
    g: 114,
    b: 128,
}; // Gray
pub const TEXT: Color = Color::Rgb {
    r: 229,
    g: 231,
    b: 235,
}; // Light gray

// ─── Styled print helpers ────────────────────────────────────────────────────

pub fn print_colored(color: Color, text: &str) {
    let mut out = io::stderr();
    let _ = out.execute(SetForegroundColor(color));
    let _ = out.execute(Print(text));
    let _ = out.execute(ResetColor);
}

pub fn print_bold(color: Color, text: &str) {
    let mut out = io::stderr();
    let _ = out.execute(SetAttribute(Attribute::Bold));
    let _ = out.execute(SetForegroundColor(color));
    let _ = out.execute(Print(text));
    let _ = out.execute(ResetColor);
    let _ = out.execute(SetAttribute(Attribute::Reset));
}

pub fn println_colored(color: Color, text: &str) {
    print_colored(color, text);
    eprintln!();
}

pub fn println_bold(color: Color, text: &str) {
    print_bold(color, text);
    eprintln!();
}

/// Print a status line: [icon] message
pub fn status(icon: &str, color: Color, msg: &str) {
    let mut out = io::stderr();
    let _ = out.execute(SetForegroundColor(DIM));
    let _ = out.execute(Print("  "));
    let _ = out.execute(SetForegroundColor(color));
    let _ = out.execute(Print(icon));
    let _ = out.execute(Print(" "));
    let _ = out.execute(SetForegroundColor(TEXT));
    let _ = out.execute(Print(msg));
    let _ = out.execute(ResetColor);
    eprintln!();
}

pub fn status_ok(msg: &str) {
    status("\u{2713}", SUCCESS, msg);
}
pub fn status_skip(msg: &str) {
    status("\u{2013}", DIM, msg);
}
pub fn status_fail(msg: &str) {
    status("\u{2717}", ERROR, msg);
}
pub fn status_warn(msg: &str) {
    status("!", WARN, msg);
}
pub fn status_work(msg: &str) {
    status("\u{25cb}", ACCENT, msg);
}

/// Print a section header
pub fn section(title: &str) {
    eprintln!();
    print_bold(BRAND, "  ");
    print_bold(TEXT, title);
    eprintln!();
    let mut out = io::stderr();
    let _ = out.execute(SetForegroundColor(DIM));
    let _ = out.execute(Print(format!(
        "  {}\n",
        "\u{2500}".repeat(title.len().min(50))
    )));
    let _ = out.execute(ResetColor);
}

// ─── Banner ──────────────────────────────────────────────────────────────────

pub fn banner() {
    let ver = env!("CARGO_PKG_VERSION");
    eprintln!();

    // W logo — left-aligned
    print_bold(BRAND, "  \\\\      //\\\\      //\n");
    print_bold(BRAND, "   \\\\    //  \\\\    //\n");
    print_bold(BRAND, "    \\\\  //    \\\\  //\n");
    print_bold(BRAND, "     \\\\//      \\\\//\n");

    eprintln!();
    print_bold(TEXT, "  W A R D E N");
    print_colored(DIM, &format!("  v{}\n", ver));
    print_colored(DIM, "  Runtime guardian for AI coding agents\n");
    eprintln!();
    print_colored(ACCENT, "  ─────────────────────────────────\n");
}

// ─── Interactive select (single) ─────────────────────────────────────────────

pub struct SelectOption {
    pub label: String,
    pub description: String,
    pub enabled: bool,
}

impl SelectOption {
    pub fn new(label: &str, desc: &str) -> Self {
        Self {
            label: label.to_string(),
            description: desc.to_string(),
            enabled: true,
        }
    }
    pub fn disabled(label: &str, desc: &str) -> Self {
        Self {
            label: label.to_string(),
            description: desc.to_string(),
            enabled: false,
        }
    }
}

/// Show an arrow-key single-select menu. Returns the index of the selected option.
pub fn select(prompt: &str, options: &[SelectOption]) -> Option<usize> {
    if options.is_empty() {
        return None;
    }

    let n = options.len();
    let mut selected = options.iter().position(|o| o.enabled).unwrap_or(0);
    let mut out = io::stderr();

    eprintln!();
    print_bold(ACCENT, "  ? ");
    print_bold(TEXT, prompt);
    eprintln!();
    print_colored(DIM, "    Use arrow keys to navigate, Enter to select\n");

    // Print initial options (reserves N lines on screen)
    render_menu(&mut out, options, selected);

    let _ = terminal::enable_raw_mode();
    let _ = write!(out, "\x1b[?25l"); // hide cursor
    let _ = out.flush();

    let result = loop {
        if let Ok(Event::Key(KeyEvent { code, kind, .. })) = event::read() {
            // Windows sends both Press and Release — only react to Press
            if kind != KeyEventKind::Press {
                continue;
            }
            match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let mut next = if selected == 0 { n - 1 } else { selected - 1 };
                    let mut attempts = 0;
                    while !options[next].enabled && attempts < n {
                        next = if next == 0 { n - 1 } else { next - 1 };
                        attempts += 1;
                    }
                    if options[next].enabled {
                        selected = next;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let mut next = (selected + 1) % n;
                    let mut attempts = 0;
                    while !options[next].enabled && attempts < n {
                        next = (next + 1) % n;
                        attempts += 1;
                    }
                    if options[next].enabled {
                        selected = next;
                    }
                }
                KeyCode::Enter => break Some(selected),
                KeyCode::Esc | KeyCode::Char('q') => break None,
                _ => continue,
            }
            let _ = write!(out, "\x1b[{}A\r", n);
            let _ = out.flush();
            render_menu(&mut out, options, selected);
        }
    };

    let _ = write!(out, "\x1b[?25h"); // show cursor
    let _ = out.flush();
    let _ = terminal::disable_raw_mode();

    if let Some(idx) = result {
        print_colored(DIM, "    Selected: ");
        println_colored(ACCENT, &options[idx].label);
    }
    eprintln!();

    result
}

/// Render menu options. Each line: clear entire line + content + \r\n.
fn render_menu(out: &mut io::Stderr, options: &[SelectOption], selected: usize) {
    for (i, opt) in options.iter().enumerate() {
        let _ = write!(out, "\x1b[2K\r");
        if !opt.enabled {
            let _ = out.execute(SetForegroundColor(DIM));
            let _ = write!(out, "      {} {}", opt.label, opt.description);
        } else if i == selected {
            let _ = out.execute(SetForegroundColor(BRAND));
            let _ = write!(out, "  \u{25b8} ");
            let _ = out.execute(SetAttribute(Attribute::Bold));
            let _ = out.execute(SetForegroundColor(TEXT));
            let _ = write!(out, "{}", opt.label);
            let _ = out.execute(SetAttribute(Attribute::Reset));
            if !opt.description.is_empty() {
                let _ = out.execute(SetForegroundColor(DIM));
                let _ = write!(out, "  {}", opt.description);
            }
        } else {
            let _ = out.execute(SetForegroundColor(DIM));
            let _ = write!(out, "    ");
            let _ = out.execute(SetForegroundColor(TEXT));
            let _ = write!(out, "{}", opt.label);
            if !opt.description.is_empty() {
                let _ = out.execute(SetForegroundColor(DIM));
                let _ = write!(out, "  {}", opt.description);
            }
        }
        let _ = out.execute(ResetColor);
        let _ = write!(out, "\r\n");
    }
    let _ = out.flush();
}

// ─── Interactive multi-select (checkboxes) ───────────────────────────────────

pub struct CheckOption {
    pub label: String,
    pub description: String,
    pub checked: bool,
    pub enabled: bool,
}

impl CheckOption {
    pub fn new(label: &str, desc: &str, default: bool) -> Self {
        Self {
            label: label.to_string(),
            description: desc.to_string(),
            checked: default,
            enabled: true,
        }
    }
    pub fn installed(label: &str, desc: &str) -> Self {
        Self {
            label: label.to_string(),
            description: desc.to_string(),
            checked: true,
            enabled: false,
        }
    }
}

/// Show an arrow-key multi-select with checkboxes.
/// Returns None on Esc, Some(indices) on Enter.
pub fn multi_select(prompt: &str, options: &mut [CheckOption]) -> Option<Vec<usize>> {
    if options.is_empty() {
        return Some(vec![]);
    }

    let n = options.len();
    let mut cur = options.iter().position(|o| o.enabled).unwrap_or(0);
    let mut out = io::stderr();

    eprintln!();
    print_bold(ACCENT, "  ? ");
    print_bold(TEXT, prompt);
    eprintln!();
    print_colored(
        DIM,
        "    Space to toggle, a to select all, Enter to confirm\n",
    );

    render_checks_menu(&mut out, options, cur);

    let _ = terminal::enable_raw_mode();
    let _ = write!(out, "\x1b[?25l");
    let _ = out.flush();

    let confirmed = loop {
        if let Ok(Event::Key(KeyEvent { code, kind, .. })) = event::read() {
            if kind != KeyEventKind::Press {
                continue;
            }
            match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let mut next = if cur == 0 { n - 1 } else { cur - 1 };
                    let mut attempts = 0;
                    while !options[next].enabled && attempts < n {
                        next = if next == 0 { n - 1 } else { next - 1 };
                        attempts += 1;
                    }
                    if options[next].enabled {
                        cur = next;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let mut next = (cur + 1) % n;
                    let mut attempts = 0;
                    while !options[next].enabled && attempts < n {
                        next = (next + 1) % n;
                        attempts += 1;
                    }
                    if options[next].enabled {
                        cur = next;
                    }
                }
                KeyCode::Char(' ') => {
                    if options[cur].enabled {
                        options[cur].checked = !options[cur].checked;
                    }
                }
                KeyCode::Char('a') => {
                    let all_checked = options.iter().filter(|o| o.enabled).all(|o| o.checked);
                    for opt in options.iter_mut().filter(|o| o.enabled) {
                        opt.checked = !all_checked;
                    }
                }
                KeyCode::Enter => break true,
                KeyCode::Esc | KeyCode::Char('q') => break false,
                _ => continue,
            }
            let _ = write!(out, "\x1b[{}A\r", n);
            let _ = out.flush();
            render_checks_menu(&mut out, options, cur);
        }
    };

    let _ = write!(out, "\x1b[?25h");
    let _ = out.flush();
    let _ = terminal::disable_raw_mode();

    if !confirmed {
        eprintln!();
        return None;
    }

    let selected: Vec<usize> = options
        .iter()
        .enumerate()
        .filter(|(_, o)| o.checked)
        .map(|(i, _)| i)
        .collect();

    let names: Vec<&str> = selected
        .iter()
        .map(|&i| options[i].label.as_str())
        .collect();
    print_colored(DIM, "    Selected: ");
    println_colored(ACCENT, &names.join(", "));
    eprintln!();

    Some(selected)
}

/// Render checkbox options. Each line: clear + content + \r\n.
fn render_checks_menu(out: &mut io::Stderr, options: &[CheckOption], cur: usize) {
    for (i, opt) in options.iter().enumerate() {
        let _ = write!(out, "\x1b[2K\r");

        let pointer = if i == cur && opt.enabled {
            "\u{25b8}"
        } else {
            " "
        };
        let checkbox = if opt.checked { "\u{25a3}" } else { "\u{25a1}" };

        if !opt.enabled {
            let _ = out.execute(SetForegroundColor(DIM));
            let _ = write!(out, "    {} {} {}", checkbox, opt.label, opt.description);
        } else if i == cur {
            let _ = out.execute(SetForegroundColor(BRAND));
            let _ = write!(out, "  {} ", pointer);
            let check_color = if opt.checked { SUCCESS } else { DIM };
            let _ = out.execute(SetForegroundColor(check_color));
            let _ = write!(out, "{} ", checkbox);
            let _ = out.execute(SetAttribute(Attribute::Bold));
            let _ = out.execute(SetForegroundColor(TEXT));
            let _ = write!(out, "{}", opt.label);
            let _ = out.execute(SetAttribute(Attribute::Reset));
            if !opt.description.is_empty() {
                let _ = out.execute(SetForegroundColor(DIM));
                let _ = write!(out, "  {}", opt.description);
            }
        } else {
            let check_color = if opt.checked { SUCCESS } else { DIM };
            let _ = write!(out, "    ");
            let _ = out.execute(SetForegroundColor(check_color));
            let _ = write!(out, "{} ", checkbox);
            let _ = out.execute(SetForegroundColor(TEXT));
            let _ = write!(out, "{}", opt.label);
            if !opt.description.is_empty() {
                let _ = out.execute(SetForegroundColor(DIM));
                let _ = write!(out, "  {}", opt.description);
            }
        }
        let _ = out.execute(ResetColor);
        let _ = write!(out, "\r\n");
    }
    let _ = out.flush();
}

// ─── Confirm (y/n with default) ──────────────────────────────────────────────

pub fn confirm(prompt: &str, default: bool) -> bool {
    let mut out = io::stderr();
    print_bold(ACCENT, "  ? ");
    print_bold(TEXT, prompt);
    let hint = if default { " (Y/n) " } else { " (y/N) " };
    print_colored(DIM, hint);
    let _ = out.flush();

    // If stdin is not a terminal (piped), fall back to line-based input
    if !is_tty() {
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
        let result = match line.trim().to_lowercase().as_str() {
            "y" | "yes" => true,
            "n" | "no" => false,
            "" => default,
            _ => default,
        };
        if result {
            println_colored(SUCCESS, "yes");
        } else {
            println_colored(DIM, "no");
        }
        return result;
    }

    let _ = terminal::enable_raw_mode();
    let result = loop {
        if let Ok(Event::Key(KeyEvent { code, kind, .. })) = event::read() {
            if kind != KeyEventKind::Press {
                continue;
            }
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => break true,
                KeyCode::Char('n') | KeyCode::Char('N') => break false,
                KeyCode::Enter => break default,
                KeyCode::Esc => break false,
                _ => {}
            }
        }
    };
    let _ = terminal::disable_raw_mode();

    if result {
        println_colored(SUCCESS, "yes");
    } else {
        println_colored(DIM, "no");
    }
    result
}

/// Check if stdin is a terminal (vs piped).
/// Uses crossterm's internal check which works cross-platform.
fn is_tty() -> bool {
    // Try enabling raw mode. If it fails, we're probably piped.
    // This is a lightweight heuristic — crossterm handles the platform details.
    // A more precise check: on Windows, try GetConsoleMode on stdin handle.
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        let handle = io::stdin().as_raw_handle();
        let mut mode = 0u32;
        // SAFETY: GetConsoleMode on stdin handle — returns 0 if not a console
        unsafe { windows_sys::Win32::System::Console::GetConsoleMode(handle as _, &mut mode) != 0 }
    }
    #[cfg(not(windows))]
    {
        // On Unix, crossterm's enable_raw_mode will fail if not a TTY,
        // but we can also just check if raw mode enable/disable succeeds
        terminal::enable_raw_mode()
            .map(|_| {
                let _ = terminal::disable_raw_mode();
                true
            })
            .unwrap_or(false)
    }
}

// ─── Spinner ─────────────────────────────────────────────────────────────────

pub struct Spinner {
    _msg: String,
}

impl Spinner {
    pub fn start(msg: &str) -> Self {
        let mut out = io::stderr();
        // Save cursor position, print spinner line
        let _ = out.execute(cursor::SavePosition);
        let _ = out.execute(SetForegroundColor(ACCENT));
        let _ = out.execute(Print(format!("  \u{25cb} {}", msg)));
        let _ = out.execute(ResetColor);
        let _ = out.flush();
        Self {
            _msg: msg.to_string(),
        }
    }

    pub fn finish_ok(self, msg: &str) {
        let mut out = io::stderr();
        // Go back to saved position, clear, write final status
        let _ = out.execute(cursor::RestorePosition);
        let _ = out.execute(terminal::Clear(ClearType::CurrentLine));
        status_ok(msg);
    }

    pub fn finish_fail(self, msg: &str) {
        let mut out = io::stderr();
        let _ = out.execute(cursor::RestorePosition);
        let _ = out.execute(terminal::Clear(ClearType::CurrentLine));
        status_fail(msg);
    }

    pub fn finish_warn(self, msg: &str) {
        let mut out = io::stderr();
        let _ = out.execute(cursor::RestorePosition);
        let _ = out.execute(terminal::Clear(ClearType::CurrentLine));
        status_warn(msg);
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Print key=value info line
pub fn info(key: &str, value: &str) {
    print_colored(DIM, &format!("    {} ", key));
    println_colored(TEXT, value);
}

/// Print a dim hint line
pub fn hint(msg: &str) {
    print_colored(DIM, "    ");
    println_colored(DIM, msg);
}
