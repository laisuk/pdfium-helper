//! Punctuation sets + helpers used by the PDF CJK reflow engine.
//!
//! Design goals
//! - Centralize punctuation / bracket / quote definitions.
//! - Keep “structure-safety” helpers (unclosed brackets, sentence boundaries)
//!   together so the reflow loop stays readable.
//!
//! NOTE: These helpers are intentionally *pessimistic* in a few places.
//! They are used for layout/reflow safety, not for perfect proofreading.

use once_cell::sync::Lazy;
use smallvec::SmallVec;
use std::collections::HashSet;

/// Broad CJK punctuation that can appear at the end of a logical unit.
///
/// Important: this set is used for *heading heuristics* and other “loose” checks.
/// It should NOT be used as a sentence-final strong boundary signal.
pub const CJK_PUNCT_END: &[char] = &[
    '。', '！', '？', '；', '：', '…', '—', '”', '」', '’', '』', '）', '】', '》', '〗', '〔',
    '〕', '〉', '⟩', '］', '｝', '》', '＞', '.', '?', '!',
];

/// Trailing brackets that may appear after a chapter marker, e.g. "第十章】".
pub const CHAPTER_TRAIL_BRACKETS: &[char] = &[
    '】', '》', '〗', '〕', '〉', '」', '』', '）', '］', '＞', '⟩',
];

pub const CHAPTER_MARKERS: &[char] = &['章', '节', '部', '卷', '節', '回'];
pub const INVALID_AFTER_MARKER: &[char] = &['分', '合'];
pub const HEADING_REJECT_PUNCT: &[char] = &['，', ',', '。', '！', '？', '；'];

pub const CJK_NUMERALS: &[char] = &['一', '二', '三', '四', '五', '六', '七', '八', '九', '十'];

pub const METADATA_SEPARATORS: &[char] = &['：', ':', '・', '　'];

pub static METADATA_KEYS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "書名",
        "书名",
        "作者",
        "譯者",
        "译者",
        "校訂",
        "校订",
        "出版社",
        "出版時間",
        "出版时间",
        "出版日期",
        "版權",
        "版权",
        "版權頁",
        "版权页",
        "版權信息",
        "版权信息",
        "責任編輯",
        "责任编辑",
        "編輯",
        "编辑",
        "責編",
        "责编",
        "定價",
        "定价",
        "前言",
        "序章",
        "終章",
        "终章",
        "尾聲",
        "尾声",
        "後記",
        "后记",
        "品牌方",
        "出品方",
        "授權方",
        "授权方",
        "電子版權",
        "数字版权",
        "掃描",
        "扫描",
        "OCR",
        "CIP",
        "在版編目",
        "在版编目",
        "分類號",
        "分类号",
        "主題詞",
        "主题词",
        "發行日",
        "发行日",
        "初版",
        "ISBN",
    ]
    .iter()
    .copied()
    .collect()
});

pub const DIALOG_OPENERS: &[char] = &['“', '‘', '「', '『', '﹁', '﹃'];

pub const DIALOG_CLOSERS: &[char] = &[
    // Standard paired closers
    '”', '’', '」', '』', '﹂', '﹄', // Occasionally seen variants / compatibility forms
    '〞', '〟',
];

#[inline]
pub fn is_dialog_opener(ch: char) -> bool {
    DIALOG_OPENERS.contains(&ch)
}

#[inline]
pub fn is_dialog_closer(ch: char) -> bool {
    DIALOG_CLOSERS.contains(&ch)
}

/// Bracket punctuations (open → close)
pub const BRACKET_PAIRS: &[(char, char)] = &[
    // Parentheses
    ('（', '）'),
    ('(', ')'),
    // Square brackets
    ('［', '］'),
    ('[', ']'),
    // Curly braces
    ('｛', '｝'),
    ('{', '}'),
    // Angle brackets
    ('＜', '＞'),
    ('<', '>'),
    ('⟨', '⟩'),
    ('〈', '〉'),
    // CJK brackets
    ('【', '】'),
    ('《', '》'),
    ('〔', '〕'),
    ('〖', '〗'),
];

#[inline]
pub fn is_bracket_opener(ch: char) -> bool {
    BRACKET_PAIRS.iter().any(|&(open, _)| open == ch)
}

#[inline]
pub fn is_bracket_closer(ch: char) -> bool {
    BRACKET_PAIRS.iter().any(|&(_, close)| close == ch)
}

#[inline]
pub fn is_allowed_postfix_closer(ch: char) -> bool {
    matches!(ch, '）' | ')')
}

#[inline]
pub fn is_matching_bracket(open: char, close: char) -> bool {
    BRACKET_PAIRS.iter().any(|&(o, c)| o == open && c == close)
}

#[inline]
pub fn is_strong_sentence_end(ch: char) -> bool {
    matches!(ch, '。' | '！' | '？' | '!' | '?')
}

#[inline]
pub fn is_comma_like(ch: char) -> bool {
    matches!(ch, '，' | ',' | '、')
}

#[inline(always)]
pub fn contains_any_comma_like(s: &str) -> bool {
    s.chars().any(is_comma_like)
}

#[inline]
pub fn is_colon_like(ch: char) -> bool {
    matches!(ch, '：' | ':')
}

#[inline]
pub fn ends_with_colon_like(s: &str) -> bool {
    let t = s.trim_end();
    t.ends_with('：') || t.ends_with(":")
}

#[inline]
pub fn ends_with_ellipsis(s: &str) -> bool {
    let t = s.trim_end();
    t.ends_with('…') || t.ends_with("……") || t.ends_with("...") || t.ends_with("..")
}

#[inline]
fn nth_char(s: &str, idx: usize) -> char {
    s.chars().nth(idx).unwrap_or('\0')
}

/// Last non-whitespace char index (char index).
pub fn find_last_non_whitespace_char_index(s: &str) -> Option<usize> {
    let mut char_pos = s.chars().count();

    for ch in s.chars().rev() {
        char_pos -= 1;
        if !ch.is_whitespace() {
            return Some(char_pos);
        }
    }
    None
}

/// Previous non-whitespace char index strictly before `end_exclusive` (char index).
pub fn find_prev_non_whitespace_char_index(s: &str, end_exclusive: usize) -> Option<usize> {
    let mut char_pos = end_exclusive;

    // IMPORTANT: reverse AFTER take() is unsafe on some toolchains,
    // so we manually limit using a counter instead.
    for ch in s.chars().rev() {
        if char_pos == 0 {
            break;
        }
        char_pos -= 1;
        if !ch.is_whitespace() {
            return Some(char_pos);
        }
    }
    None
}

/// Minimal CJK checker (BMP focused).
/// Designed for heading / structure heuristics, not full Unicode linguistics.
#[inline]
pub fn is_cjk_bmp(ch: char) -> bool {
    let c = ch as u32;
    (0x3400..=0x4DBF).contains(&c)
        || (0x4E00..=0x9FFF).contains(&c)
        || (0xF900..=0xFAFF).contains(&c)
}

#[inline]
pub fn is_digit_ascii_or_fullwidth(ch: char) -> bool {
    // ASCII digits
    if ('0'..='9').contains(&ch) {
        return true;
    }
    // FULLWIDTH digits
    ('０'..='９').contains(&ch)
}

/// “Mostly CJK” heuristic used by a few boundary rules.
///
/// - Counts CJK letters as CJK.
/// - Counts ASCII alphabetic letters as ASCII.
/// - Treats digits and whitespace as neutral.
#[inline]
pub fn is_mostly_cjk(s: &str) -> bool {
    let mut cjk = 0usize;
    let mut ascii = 0usize;

    for ch in s.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if is_digit_ascii_or_fullwidth(ch) {
            continue;
        }

        if is_cjk_bmp(ch) {
            cjk += 1;
            continue;
        }

        // Count ASCII letters only; ASCII punctuation is neutral
        if ch <= '\u{7F}' && ch.is_ascii_alphabetic() {
            ascii += 1;
        }
    }

    cjk > 0 && cjk >= ascii
}

/// Cross-page / soft-wrap safety:
/// If the previous buffer is inside an unclosed bracket like
/// "（......" ... "...。）", never flush on blank lines / weak boundaries.
///
/// NOTE: intentionally pessimistic.
/// - Any stray closer is treated as unsafe.
/// - Any mismatch is treated as unsafe.
#[inline]
pub fn has_unclosed_bracket(s: &str) -> bool {
    let mut stack: SmallVec<[char; 4]> = SmallVec::new();
    let mut seen_bracket = false;

    for ch in s.chars() {
        if is_bracket_opener(ch) {
            seen_bracket = true;
            stack.push(ch);
            continue;
        }

        if is_bracket_closer(ch) {
            seen_bracket = true;

            // STRICT: stray closer = unsafe
            let open = match stack.pop() {
                Some(o) => o,
                None => return true,
            };

            if !is_matching_bracket(open, ch) {
                return true;
            }
        }
    }

    seen_bracket && !stack.is_empty()
}

// ------ Sentence Boundary start ------ //

/// Level-2 normalized sentence boundary detection.
///
/// Includes OCR artifacts (ASCII '.' / ':'), but **does not** treat a bare
/// bracket closer as a sentence boundary (that causes false flushes like "（亦作肥）").
pub fn ends_with_sentence_boundary(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }

    let last_non_ws = match find_last_non_whitespace_char_index(s) {
        Some(i) => i,
        None => return false,
    };

    let last = nth_char(s, last_non_ws);

    // 1) Strong sentence enders.
    if is_strong_sentence_end(last) {
        return true;
    }

    // 2) OCR '.' / ':' at line end (mostly-CJK).
    if (last == '.' || last == ':') && is_ocr_cjk_ascii_punct_at_line_end(s, last_non_ws) {
        return true;
    }

    // 3) Quote closers + Allowed postfix closer after strong end, plus OCR artifact `.“”` / `.」` / `.）`.
    if is_dialog_closer(last) || is_allowed_postfix_closer(last) {
        if let Some(prev_non_ws) = find_prev_non_whitespace_char_index(s, last_non_ws) {
            let prev = nth_char(s, prev_non_ws);

            if is_strong_sentence_end(prev) {
                return true;
            }

            if prev == '.' && is_ocr_cjk_ascii_punct_before_closers(s, prev_non_ws) {
                return true;
            }
        }
    }

    // 4.0) Bracket closers with mostly CJK. (reserved)
    // if is_bracket_closer(last) && last_non_ws > 0 && is_mostly_cjk(s) {
    //     return true;
    // }

    // 4) Full-width colon as a weak boundary (common: "他说：" then dialog next line)
    if is_colon_like(last) && is_mostly_cjk(s) {
        return true;
    }

    // 5) Ellipsis as weak boundary.
    if ends_with_ellipsis(s) {
        return true;
    }

    false
}

/// Strict OCR: punct itself is at end-of-line (only whitespace after it),
/// and preceded by CJK in a mostly-CJK line.
fn is_ocr_cjk_ascii_punct_at_line_end(s: &str, punct_index: usize) -> bool {
    if punct_index == 0 {
        return false;
    }
    if !is_at_line_end_ignoring_whitespace(s, punct_index) {
        return false;
    }
    let prev = nth_char(s, punct_index - 1);
    is_cjk_bmp(prev) && is_mostly_cjk(s)
}

/// Relaxed OCR: after punct, allow only whitespace and closers (quote/bracket).
/// This enables `“.”` / `.」` / `.）` to count as sentence boundary.
fn is_ocr_cjk_ascii_punct_before_closers(s: &str, punct_index: usize) -> bool {
    if punct_index == 0 {
        return false;
    }
    if !is_at_end_allowing_closers(s, punct_index) {
        return false;
    }
    let prev = nth_char(s, punct_index - 1);
    is_cjk_bmp(prev) && is_mostly_cjk(s)
}

fn is_at_line_end_ignoring_whitespace(s: &str, index: usize) -> bool {
    s.chars().skip(index + 1).all(|c| c.is_whitespace())
}

fn is_at_end_allowing_closers(s: &str, index: usize) -> bool {
    for ch in s.chars().skip(index + 1) {
        if ch.is_whitespace() {
            continue;
        }
        if is_dialog_closer(ch) || is_bracket_closer(ch) {
            continue;
        }
        return false;
    }
    true
}

// ------ Sentence Boundary end ------ //

// ------ Bracket Boundary start ------ //

/// Returns true if the string ends with a balanced CJK-style bracket boundary,
/// e.g. （完）, 【番外】, 《後記》.
pub fn ends_with_cjk_bracket_boundary(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }

    // Need at least two chars: open + close
    let mut chars = t.chars();
    let open = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    let close = match t.chars().rev().next() {
        Some(c) => c,
        None => return false,
    };

    // 1) Must be a known matching pair
    if !is_matching_bracket(open, close) {
        return false;
    }

    // 2) Avoid Latin cases like "(test)" or "[1.2]"
    if !is_mostly_cjk(t) {
        return false;
    }

    // 3) Ensure this bracket type is balanced inside the string
    is_bracket_type_balanced(t, open, close)
}

#[inline]
fn is_bracket_type_balanced(s: &str, open: char, close: char) -> bool {
    let mut depth: i32 = 0;

    for ch in s.chars() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth < 0 {
                return false;
            }
        }
    }

    depth == 0
}

// ------ Bracket Boundary end ------ //
