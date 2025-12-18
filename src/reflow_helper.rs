//! CJK PDF Reflow Engine (pure Rust)
//!
//! Extracted from opencc_pyo3 PyO3 module. Designed to be reused by:
//! - Python bindings (thin wrapper)
//! - CLI (opencc-rs pdf / office / etc.)

use once_cell::sync::Lazy;
use std::collections::HashSet;

/// Reflow CJK paragraphs from PDF-extracted text.
///
/// Mirrors the behavior of the original `reflow_cjk_paragraphs()` PyO3 function.
/// - Normalizes CRLF/CR to LF
/// - Merges artificial line breaks
/// - Preserves headings / metadata / page markers / dialog structure
///
/// `add_pdf_page_header`:
/// - If false, skips page-break-like blank lines not preceded by CJK punctuation.
///
/// `compact`:
/// - If true, join paragraphs with "\n"
/// - If false, join paragraphs with "\n\n"
pub fn reflow_cjk_paragraphs(text: &str, add_pdf_page_header: bool, compact: bool) -> String {
    // If the whole text is whitespace, return as-is.
    if text.chars().all(|c| c.is_whitespace()) {
        return text.to_owned();
    }

    // Normalize line endings
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized.split('\n');

    let mut segments: Vec<String> = Vec::new();
    let mut buffer = String::new();
    let mut dialog_state = DialogState::new();

    for raw_line in lines {
        // 1) Visual form: trim right-side whitespace, then remove halfwidth indent
        let trimmed_end = raw_line.trim_end();
        let stripped_visual = strip_halfwidth_indent_keep_fullwidth(trimmed_end);

        // 1.1) Logical probe for heading detection (no left indent)
        let probe = stripped_visual.trim_start_matches(|ch| ch == ' ' || ch == '\u{3000}');

        // 1.2 Visual divider line (box drawing / ---- / === / *** / ★★★ etc.)
        // Always force paragraph breaks.
        if is_box_drawing_line(probe) {
            if !buffer.is_empty() {
                segments.push(std::mem::take(&mut buffer));
                dialog_state.reset();
            }
            segments.push(stripped_visual.to_string());
            continue;
        }

        // 2) Collapse style-layer repeated segments per line
        let line_text = collapse_repeated_segments(stripped_visual);

        // 3) Logical probe for heading detection (no left indent)
        let heading_probe = line_text.trim_start_matches(|ch| ch == ' ' || ch == '\u{3000}');

        // 4) Empty line handling
        if heading_probe.trim().is_empty() {
            if !add_pdf_page_header && !buffer.is_empty() {
                // Skip soft page breaks if previous paragraph does not end with CJK punctuation.
                if let Some(last_char) = buffer.chars().rev().find(|c| !c.is_whitespace()) {
                    if !CJK_PUNCT_END.contains(&last_char) {
                        continue;
                    }
                }
            }

            // End paragraph → flush buffer (do not emit empty segments)
            if !buffer.is_empty() {
                segments.push(std::mem::take(&mut buffer));
                dialog_state.reset();
            }
            continue;
        }

        // 5) Page marker lines
        if is_page_marker(heading_probe) {
            if !buffer.is_empty() {
                segments.push(std::mem::take(&mut buffer));
                dialog_state.reset();
            }
            segments.push(line_text.clone());
            continue;
        }

        // 6) Heading / metadata detection
        let is_title_heading = is_title_heading_line(heading_probe);
        let is_short_heading = is_heading_like(&line_text);
        let is_metadata = is_metadata_line(&line_text);

        let mut flush_buffer_and_push_line = |line: &str| {
            if !buffer.is_empty() {
                segments.push(std::mem::take(&mut buffer));
                dialog_state.reset();
            }
            segments.push(line.to_owned());
        };

        if is_metadata {
            flush_buffer_and_push_line(&line_text);
            continue;
        }
        if is_title_heading {
            flush_buffer_and_push_line(&line_text);
            continue;
        }

        if is_short_heading {
            let stripped = heading_probe;

            if !buffer.is_empty() {
                let buf_text = buffer.as_str();

                if has_unclosed_bracket(buf_text) {
                    // treat as continuation
                } else {
                    let bt = buf_text.trim_end();
                    if let Some(last) = bt.chars().last() {
                        if last == '，' || last == ',' || last == '、' {
                            // continuation
                        } else {
                            let is_all_cjk = is_all_cjk_ignoring_ws(stripped);
                            if is_all_cjk && !CJK_PUNCT_END.contains(&last) {
                                // continuation
                            } else {
                                segments.push(std::mem::take(&mut buffer));
                                dialog_state.reset();
                                segments.push(line_text.clone());
                                continue;
                            }
                        }
                    } else {
                        segments.push(line_text.clone());
                        continue;
                    }
                }
            } else {
                segments.push(line_text.clone());
                continue;
            }
        }

        // 7) Dialog detection
        let current_is_dialog_start = is_dialog_start(&line_text);

        // First line of a new paragraph
        if buffer.is_empty() {
            buffer.push_str(&line_text);
            dialog_state.reset();
            dialog_state.update(&line_text);
            continue;
        }

        let buffer_text = buffer.as_str();

        // If previous line ends with comma, do NOT flush even if new line starts dialog
        if current_is_dialog_start {
            let trimmed_buffer = buffer_text.trim_end();
            let last = trimmed_buffer.chars().rev().next();
            if let Some(ch) = last {
                if ch != '，' && ch != ',' {
                    segments.push(std::mem::take(&mut buffer));
                    buffer.push_str(&line_text);
                    dialog_state.reset();
                    dialog_state.update(&line_text);
                    continue;
                }
            } else {
                segments.push(std::mem::take(&mut buffer));
                buffer.push_str(&line_text);
                dialog_state.reset();
                dialog_state.update(&line_text);
                continue;
            }
        }

        // Colon + dialog continuation
        if let Some(last_char) = buffer_text.chars().rev().find(|c| !c.is_whitespace()) {
            if last_char == '：' || last_char == ':' {
                let after_indent =
                    line_text.trim_start_matches(|ch| ch == ' ' || ch == '\u{3000}');
                if let Some(first_ch) = after_indent.chars().next() {
                    if DIALOG_OPENERS.contains(&first_ch) {
                        buffer.push_str(&line_text);
                        dialog_state.update(&line_text);
                        continue;
                    }
                }
            }
        }

        // 8) Punct-driven boundary
        if buffer_ends_with_cjk_punct(buffer_text) && !dialog_state.is_unclosed() {
            segments.push(std::mem::take(&mut buffer));
            buffer.push_str(&line_text);
            dialog_state.reset();
            dialog_state.update(&line_text);
            continue;
        }

        // 9) Chapter-like ending lines
        if is_chapter_ending_line(buffer_text) {
            segments.push(std::mem::take(&mut buffer));
            buffer.push_str(&line_text);
            dialog_state.reset();
            dialog_state.update(&line_text);
            continue;
        }

        // 10) Default soft join
        buffer.push_str(&line_text);
        dialog_state.update(&line_text);
    }

    if !buffer.is_empty() {
        segments.push(buffer);
    }

    if compact {
        segments.join("\n")
    } else {
        segments.join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// Constants and helpers (copied from your original)
// ---------------------------------------------------------------------------

const CJK_PUNCT_END: &[char] = &[
    '。', '！', '？', '；', '：', '…', '—', '”', '」', '’', '』', '）', '】', '》', '〗', '〔',
    '〕', '〉', '］', '｝', '》', '.', '?', '!',
];

const CHAPTER_TRAIL_BRACKETS: &[char] = &['】', '》', '〗', '〕', '〉', '」', '』', '）'];

const HEADING_KEYWORDS: &[&str] = &["前言", "序章", "终章", "尾声", "后记", "番外", "尾聲", "後記"];

const CHAPTER_MARKERS: &[char] = &['章', '节', '部', '卷', '節', '回'];
const INVALID_AFTER_MARKER: &[char] = &['分', '合'];

const METADATA_SEPARATORS: &[char] = &['：', ':', '　'];

static METADATA_KEYS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "書名", "书名", "作者", "譯者", "译者", "校訂", "校订", "出版社", "出版時間", "出版时间",
        "出版日期", "版權", "版权", "版權頁", "版权页", "版權信息", "版权信息", "責任編輯", "责任编辑",
        "編輯", "编辑", "責編", "责编", "定價", "定价", "前言", "序章", "終章", "终章", "尾聲",
        "尾声", "後記", "后记", "品牌方", "出品方", "授權方", "授权方", "電子版權", "数字版权", "掃描",
        "扫描", "OCR", "CIP", "在版編目", "在版编目", "分類號", "分类号", "主題詞", "主题词", "發行日",
        "发行日", "初版", "ISBN",
    ]
        .iter()
        .copied()
        .collect()
});

const DIALOG_OPENERS: &[char] = &['“', '‘', '「', '『', '﹁', '﹃'];

const OPEN_BRACKETS: &[char] = &['（', '(', '[', '【', '《', '<', '{'];
const CLOSE_BRACKETS: &[char] = &['）', ')', ']', '】', '》', '>', '}'];

fn is_metadata_line(line: &str) -> bool {
    let s = line.trim();
    if s.is_empty() || s.chars().count() > 30 {
        return false;
    }

    let mut char_pos = 0usize;
    let mut sep_byte_idx: Option<usize> = None;

    for (byte_idx, ch) in s.char_indices() {
        if METADATA_SEPARATORS.contains(&ch) {
            if char_pos == 0 || char_pos > 10 {
                return false;
            }
            sep_byte_idx = Some(byte_idx);
            break;
        }
        char_pos += 1;
    }

    let sep_byte_idx = match sep_byte_idx {
        Some(idx) => idx,
        None => return false,
    };

    let key = s[..sep_byte_idx].trim();
    if !METADATA_KEYS.contains(key) {
        return false;
    }

    let sep_char = s[sep_byte_idx..].chars().next().unwrap();
    let after_sep = sep_byte_idx + sep_char.len_utf8();

    let mut found_next: Option<char> = None;
    for (_, ch) in s[after_sep..].char_indices() {
        if ch.is_whitespace() {
            continue;
        }
        found_next = Some(ch);
        break;
    }

    let first_after = match found_next {
        Some(ch) => ch,
        None => return false,
    };

    !DIALOG_OPENERS.contains(&first_after)
}

#[inline]
pub fn is_box_drawing_line(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }

    let mut total = 0usize;

    for ch in s.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;

        match ch {
            '\u{2500}'..='\u{257F}' => {}
            '-' | '=' | '_' | '~' | '～' => {}
            '*' | '＊' | '★' | '☆' => {}
            _ => return false,
        }
    }

    total >= 3
}

fn buffer_ends_with_cjk_punct(s: &str) -> bool {
    s.chars()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_some_and(|ch| CJK_PUNCT_END.contains(&ch))
}

fn is_page_marker(s: &str) -> bool {
    s.starts_with("=== ") && s.ends_with("===")
}

fn is_title_heading_line(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    let char_count = s.chars().count();
    if char_count > 50 {
        return false;
    }

    for &kw in HEADING_KEYWORDS {
        if s.starts_with(kw) {
            return true;
        }
    }

    if let Some(rest) = s.strip_prefix("番外") {
        return rest.chars().count() <= 15;
    }

    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        if chars[i] != '第' {
            continue;
        }
        if i > 10 {
            continue;
        }

        for j in (i + 1)..chars.len() {
            if j - i > 6 {
                break;
            }
            let ch = chars[j];
            if !CHAPTER_MARKERS.contains(&ch) {
                continue;
            }

            if let Some(next) = chars.get(j + 1) {
                if INVALID_AFTER_MARKER.contains(next) {
                    return false;
                }
            }

            if chars.len().saturating_sub(j + 1) <= 20 {
                return true;
            }
        }
    }

    false
}

fn is_chapter_ending_line(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.chars().count() > 15 {
        return false;
    }

    let mut trimmed = s;
    loop {
        if let Some(last) = trimmed.chars().last() {
            if CHAPTER_TRAIL_BRACKETS.contains(&last) {
                let new_len = trimmed.len() - last.len_utf8();
                trimmed = &trimmed[..new_len];
                continue;
            }
        }
        break;
    }

    trimmed
        .chars()
        .last()
        .is_some_and(|last| CHAPTER_MARKERS.contains(&last))
}

fn is_dialog_start(s: &str) -> bool {
    let trimmed = s.trim_start_matches(|ch| ch == ' ' || ch == '\u{3000}');
    trimmed
        .chars()
        .next()
        .is_some_and(|ch| DIALOG_OPENERS.contains(&ch))
}

fn is_heading_like(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    if s.starts_with("=== ") && s.ends_with("===") {
        return false;
    }

    let has_open = s.chars().any(|ch| OPEN_BRACKETS.contains(&ch));
    let has_close = s.chars().any(|ch| CLOSE_BRACKETS.contains(&ch));
    if has_open && !has_close {
        return false;
    }

    let len = s.chars().count();
    let max_len = if is_all_ascii(s) { 16 } else { 8 };

    if let Some(last) = s.chars().last() {
        if (last == '：' || last == ':') && len < max_len {
            let body = strip_last_char(s);
            if is_all_cjk(body) {
                return true;
            }
        }
        if CJK_PUNCT_END.contains(&last) {
            return false;
        }
    }

    if s.contains('，') || s.contains(',') || s.contains('、') {
        return false;
    }

    if len <= max_len {
        if s.chars().any(|ch| CJK_PUNCT_END.contains(&ch)) {
            return false;
        }

        let mut has_non_ascii = false;
        let mut all_ascii = true;
        let mut has_letter = false;
        let mut all_ascii_digits = true;

        for ch in s.chars() {
            if (ch as u32) > 0x7F {
                has_non_ascii = true;
                all_ascii = false;
                all_ascii_digits = false;
                continue;
            }
            if !ch.is_ascii_digit() {
                all_ascii_digits = false;
            }
            if ch.is_ascii_alphabetic() {
                has_letter = true;
            }
        }

        if all_ascii_digits {
            return true;
        }
        if has_non_ascii {
            return true;
        }
        if all_ascii && has_letter {
            return true;
        }
    }

    false
}

#[inline]
pub fn is_all_ascii(s: &str) -> bool {
    s.is_ascii()
}

#[inline]
pub fn is_all_cjk(s: &str) -> bool {
    let mut any = false;
    for ch in s.chars() {
        any = true;
        if ch.is_whitespace() {
            return false;
        }
        if !is_cjk_bmp(ch) {
            return false;
        }
    }
    any
}

#[inline]
pub fn is_cjk_bmp(ch: char) -> bool {
    let c = ch as u32;
    (0x3400..=0x4DBF).contains(&c) || (0x4E00..=0x9FFF).contains(&c) || (0xF900..=0xFAFF).contains(&c)
}

#[inline]
fn is_all_cjk_ignoring_ws(s: &str) -> bool {
    let mut any = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            continue;
        }
        any = true;
        if (ch as u32) <= 0x7F {
            return false;
        }
    }
    any
}

#[inline]
pub fn has_unclosed_bracket(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut has_open = false;
    let mut has_close = false;

    for ch in s.chars() {
        if !has_open && OPEN_BRACKETS.contains(&ch) {
            has_open = true;
        }
        if !has_close && CLOSE_BRACKETS.contains(&ch) {
            has_close = true;
        }
        if has_open && has_close {
            break;
        }
    }

    has_open && !has_close
}

fn collapse_repeated_segments(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return line.to_owned();
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return line.to_owned();
    }

    let phrase_collapsed = collapse_repeated_word_sequences(&parts);
    let token_collapsed: Vec<String> = phrase_collapsed
        .into_iter()
        .map(|tok| collapse_repeated_token(&tok))
        .collect();

    token_collapsed.join(" ")
}

fn collapse_repeated_word_sequences(parts: &[&str]) -> Vec<String> {
    const MIN_REPEATS: usize = 3;
    const MAX_PHRASE_LEN: usize = 8;

    let n = parts.len();
    if n < MIN_REPEATS {
        return parts.iter().map(|s| (*s).to_owned()).collect();
    }

    for start in 0..n {
        for phrase_len in 1..=MAX_PHRASE_LEN {
            if start + phrase_len > n {
                break;
            }

            let mut count = 1;

            loop {
                let next_start = start + count * phrase_len;
                if next_start + phrase_len > n {
                    break;
                }

                let mut equal = true;
                for k in 0..phrase_len {
                    if parts[start + k] != parts[next_start + k] {
                        equal = false;
                        break;
                    }
                }
                if !equal {
                    break;
                }
                count += 1;
            }

            if count >= MIN_REPEATS {
                let mut result = Vec::with_capacity(n - (count - 1) * phrase_len);
                for i in 0..start {
                    result.push(parts[i].to_owned());
                }
                for k in 0..phrase_len {
                    result.push(parts[start + k].to_owned());
                }
                let tail_start = start + count * phrase_len;
                for i in tail_start..n {
                    result.push(parts[i].to_owned());
                }
                return result;
            }
        }
    }

    parts.iter().map(|s| (*s).to_owned()).collect()
}

fn collapse_repeated_token(token: &str) -> String {
    let chars: Vec<char> = token.chars().collect();
    let length = chars.len();

    if length < 4 || length > 200 {
        return token.to_owned();
    }

    for unit_len in 4..=10 {
        if unit_len > length / 3 {
            break;
        }
        if length % unit_len != 0 {
            continue;
        }

        let unit = &chars[0..unit_len];
        let repeat_count = length / unit_len;

        let mut all_match = true;
        for i in 1..repeat_count {
            let start = i * unit_len;
            let end = start + unit_len;
            if &chars[start..end] != unit {
                all_match = false;
                break;
            }
        }

        if all_match {
            return unit.iter().collect();
        }
    }

    token.to_owned()
}

struct DialogState {
    double_quote: i32,
    single_quote: i32,
    corner: i32,
    corner_bold: i32,
    corner_top: i32,
    corner_wide: i32,
}

impl DialogState {
    fn new() -> Self {
        Self {
            double_quote: 0,
            single_quote: 0,
            corner: 0,
            corner_bold: 0,
            corner_top: 0,
            corner_wide: 0,
        }
    }

    fn reset(&mut self) {
        self.double_quote = 0;
        self.single_quote = 0;
        self.corner = 0;
        self.corner_bold = 0;
        self.corner_top = 0;
        self.corner_wide = 0;
    }

    fn update(&mut self, s: &str) {
        for ch in s.chars() {
            match ch {
                '“' => self.double_quote += 1,
                '”' => self.double_quote = (self.double_quote - 1).max(0),
                '‘' => self.single_quote += 1,
                '’' => self.single_quote = (self.single_quote - 1).max(0),
                '「' => self.corner += 1,
                '」' => self.corner = (self.corner - 1).max(0),
                '『' => self.corner_bold += 1,
                '』' => self.corner_bold = (self.corner_bold - 1).max(0),
                '﹁' => self.corner_top += 1,
                '﹂' => self.corner_top = (self.corner_top - 1).max(0),
                '﹃' => self.corner_wide += 1,
                '﹄' => self.corner_wide = (self.corner_wide - 1).max(0),
                _ => {}
            }
        }
    }

    fn is_unclosed(&self) -> bool {
        self.double_quote > 0
            || self.single_quote > 0
            || self.corner > 0
            || self.corner_bold > 0
            || self.corner_top > 0
            || self.corner_wide > 0
    }
}

fn strip_halfwidth_indent_keep_fullwidth(s: &str) -> &str {
    let mut start_byte = 0;
    for (idx, ch) in s.char_indices() {
        if ch == ' ' {
            start_byte = idx + ch.len_utf8();
            continue;
        }
        break;
    }
    &s[start_byte..]
}

fn strip_last_char(s: &str) -> &str {
    match s.char_indices().last() {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
