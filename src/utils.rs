use std::io::{self, Write};

/// Format integer with thousands separators (Python {:,} equivalent)
pub fn format_thousand(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);

    let bytes = s.as_bytes();
    let len = bytes.len();

    for (i, &b) in bytes.iter().enumerate() {
        out.push(b as char);

        let remaining = len - i - 1;
        if remaining > 0 && remaining % 3 == 0 {
            out.push(',');
        }
    }

    out
}

/// Print in-place of progress line (carriage-return redraw).
pub fn print_progress(page: i32, total: i32, text: &str) {
    let percent = page * 100 / total.max(1);

    let msg = format!(
        "Loading [{}/{}] ({:3}%) Extracted {} chars",
        page,
        total,
        percent,
        text.chars().count()
    );

    // pad to fully overwrite previous line
    let mut line = msg;
    if line.len() < 80 {
        line.push_str(&" ".repeat(80 - line.len()));
    }

    print!("\r{}", line);
    let _ = io::stdout().flush();
}
