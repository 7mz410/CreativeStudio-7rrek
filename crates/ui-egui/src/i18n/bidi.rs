//! Creative Studio: right-to-left catalogs. egui shapes each word (so Arabic letters join) but lays the
//! words of a line out left to right, so a right-to-left translation is stored with its words
//! already in visual order: the Unicode bidirectional algorithm splits each line into runs, and
//! the words inside every right-to-left run are reversed. Letters inside a word keep their
//! logical order for egui to shape.

use unicode_bidi::{BidiInfo, Level};

/// Languages whose catalogs are stored in visual order.
pub const RTL_CODES: &[&str] = &["ar"];

/// A catalog's source with every translation (third column) put in visual order.
pub fn visual_catalog(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            if line.starts_with('#') {
                return line.to_string();
            }
            let mut cells: Vec<String> = line.split('\t').map(str::to_string).collect();
            if let Some(tr) = cells.get_mut(2) {
                // Plural forms are separated by `|`; each form is ordered on its own.
                *tr = tr.split('|').map(visual).collect::<Vec<_>>().join("|");
            }
            cells.join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `s` in visual order, line by line (`\n` is written escaped in catalogs, so it is kept).
pub fn visual(s: &str) -> String {
    s.split("\\n").map(visual_line).collect::<Vec<_>>().join("\\n")
}

fn is_rtl(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

fn visual_line(line: &str) -> String {
    if !line.chars().any(is_rtl) {
        return line.to_string();
    }
    // Leading and trailing spaces join fragments at runtime, so they stay where they are.
    let core = line.trim_matches(' ');
    let lead = &line[..line.len() - line.trim_start_matches(' ').len()];
    let trail = &line[line.trim_end_matches(' ').len()..];
    format!("{lead}{}{trail}", visual_core(core))
}

fn visual_core(line: &str) -> String {
    // Sizes such as 1920×1080 read left to right: the bidi analysis sees a strong left-to-right
    // letter of the same byte length in place of a `×` between digits, so the size stays one run.
    let analysed = keep_sizes_together(line);
    let info = BidiInfo::new(&analysed, Some(Level::rtl()));
    let Some(para) = info.paragraphs.first() else { return line.to_string() };
    let (levels, runs) = info.visual_runs(para, para.range.clone());
    let mut out = String::with_capacity(line.len());
    for run in runs {
        let text = &line[run.clone()];
        if levels[run.start].is_rtl() {
            let mut tokens = tokens(text);
            tokens.reverse();
            out.extend(tokens);
        } else {
            out.push_str(text);
        }
    }
    out
}

fn keep_sizes_together(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    // Placeholders such as `{n}` are filled in after the catalog is read, so each stays one
    // left-to-right run: its braces count as Latin letters of the same byte length.
    let mut in_placeholder = vec![false; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{'
            && let Some(len) = chars[i + 1..].iter().position(|&c| c == '}')
        {
            let name = &chars[i + 1..i + 1 + len];
            if !name.is_empty() && name.iter().all(|c| c.is_ascii_alphanumeric() || *c == '_') {
                in_placeholder[i..=i + 1 + len].fill(true);
                i += len + 2;
                continue;
            }
        }
        i += 1;
    }
    chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let between_digits = i > 0 && i + 1 < chars.len() && chars[i - 1].is_ascii_digit() && chars[i + 1].is_ascii_digit();
            if c == '×' && between_digits {
                'é'
            } else if in_placeholder[i] && (c == '{' || c == '}') {
                'a'
            } else {
                c
            }
        })
        .collect()
}

/// The pieces of a right-to-left run whose order flips: right-to-left words, whitespace, and each
/// other character on its own (Latin punctuation such as `.` or `(` would otherwise stick to the
/// wrong side of its word).
fn tokens(s: &str) -> Vec<&str> {
    #[derive(PartialEq, Clone, Copy)]
    enum Kind {
        Word,
        Space,
        Other,
    }
    let kind = |c: char| {
        if is_rtl(c) {
            Kind::Word
        } else if c.is_whitespace() {
            Kind::Space
        } else {
            Kind::Other
        }
    };
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev: Option<Kind> = None;
    for (i, c) in s.char_indices() {
        let k = kind(c);
        if prev.is_some_and(|p| p != k || k == Kind::Other) {
            out.push(&s[start..i]);
            start = i;
        }
        prev = Some(k);
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_words_are_reversed_latin_kept() {
        assert_eq!(visual("ملف جديد"), "جديد ملف");
        assert_eq!(visual("أهلاً بك في rsm"), "rsm في بك أهلاً");
        assert_eq!(visual("Open"), "Open");
        // A placeholder stays whole so it can still be filled in.
        assert_eq!(visual("{n} طبقة"), "طبقة {n}");
        assert_eq!(visual("حذف المكتبة “{name}”؟"), "؟”{name}“ المكتبة حذف");
        assert_eq!(visual("قابل للبرمجة."), ".للبرمجة قابل");
        assert_eq!(visual(" للتحريك  |  "), " |  للتحريك  ");
        assert_eq!(visual("ويب 1920×1080"), "1920×1080 ويب");
    }

    #[test]
    fn catalog_rows_keep_their_columns() {
        assert_eq!(visual_catalog("# ملف جديد\n\tNew file\tملف جديد"), "# ملف جديد\n\tNew file\tجديد ملف");
    }
}
