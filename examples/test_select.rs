use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Attribute, Color, ResetColor, SetAttribute, SetForegroundColor},
    terminal,
};
use std::io::{self, Write};

const BRAND: Color = Color::Rgb {
    r: 220,
    g: 38,
    b: 38,
};
const DIM: Color = Color::Rgb {
    r: 107,
    g: 114,
    b: 128,
};
const TEXT: Color = Color::Rgb {
    r: 229,
    g: 231,
    b: 235,
};

fn main() {
    let options = vec![
        ("Claude Code", "Configure hooks for Claude Code"),
        ("Gemini CLI", "Google's AI coding assistant"),
        ("Skip", "Configure later"),
    ];

    let n = options.len();
    let mut selected: usize = 0;
    let mut out = io::stderr();

    eprintln!("\n  Test: arrow-key select ({} options)", n);
    eprintln!("  Press arrows, Enter, or Esc. Debug info shown below.\n");

    render(&mut out, &options, selected);

    terminal::enable_raw_mode().expect("enable raw mode");
    let _ = write!(out, "\x1b[?25l");
    let _ = out.flush();

    let result = loop {
        match event::read() {
            Ok(Event::Key(KeyEvent {
                code,
                modifiers,
                kind,
                ..
            })) => {
                // Only react to Press events (ignore Release on Windows)
                if kind != crossterm::event::KeyEventKind::Press {
                    continue;
                }

                match code {
                    KeyCode::Up => {
                        selected = if selected == 0 { n - 1 } else { selected - 1 };
                    }
                    KeyCode::Down => {
                        selected = (selected + 1) % n;
                    }
                    KeyCode::Enter => break Some(selected),
                    KeyCode::Esc => break None,
                    other => {
                        // Debug: show what key was pressed
                        let _ = write!(out, "\x1b[{}B\r", n - selected);
                        let _ =
                            write!(out, "\x1b[2K\r  debug: key={:?} mod={:?}", other, modifiers);
                        let _ = write!(out, "\x1b[{}A\r", n - selected);
                        let _ = out.flush();
                        continue;
                    }
                }
                let _ = write!(out, "\x1b[{}A\r", n);
                let _ = out.flush();
                render(&mut out, &options, selected);
            }
            Ok(other_event) => {
                // Debug: show non-key events
                let _ = write!(out, "\x1b[{}B\r", n);
                let _ = write!(out, "\x1b[2K\r  debug: event={:?}", other_event);
                let _ = write!(out, "\x1b[{}A\r", n);
                let _ = out.flush();
                continue;
            }
            Err(e) => {
                let _ = write!(out, "\r\n  error: {}\r\n", e);
                let _ = out.flush();
                break None;
            }
        }
    };

    let _ = write!(out, "\x1b[?25h");
    let _ = out.flush();
    terminal::disable_raw_mode().expect("disable raw mode");

    eprintln!();
    match result {
        Some(idx) => eprintln!("  Selected: {} (index {})", options[idx].0, idx),
        None => eprintln!("  Cancelled (Esc)"),
    }
    eprintln!();
}

fn render(out: &mut io::Stderr, options: &[(&str, &str)], selected: usize) {
    for (i, (label, desc)) in options.iter().enumerate() {
        let _ = write!(out, "\x1b[2K\r");
        if i == selected {
            let _ = out.execute(SetForegroundColor(BRAND));
            let _ = write!(out, "  \u{25b8} ");
            let _ = out.execute(SetAttribute(Attribute::Bold));
            let _ = out.execute(SetForegroundColor(TEXT));
            let _ = write!(out, "{}", label);
            let _ = out.execute(SetAttribute(Attribute::Reset));
            let _ = out.execute(SetForegroundColor(DIM));
            let _ = write!(out, "  {}", desc);
        } else {
            let _ = out.execute(SetForegroundColor(DIM));
            let _ = write!(out, "    ");
            let _ = out.execute(SetForegroundColor(TEXT));
            let _ = write!(out, "{}", label);
            let _ = out.execute(SetForegroundColor(DIM));
            let _ = write!(out, "  {}", desc);
        }
        let _ = out.execute(ResetColor);
        let _ = write!(out, "\r\n");
    }
    let _ = out.flush();
}
