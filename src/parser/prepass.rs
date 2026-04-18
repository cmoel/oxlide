use super::ast::{ParseError, SourceSpan};

pub const SLIDE_BREAK_SENTINEL: &str = "<!-- oxlide-slide-break -->";
pub const CELL_BREAK_SENTINEL: &str = "<!-- oxlide-cell-break -->";
pub const IMAGE_META_SENTINEL_NAME: &str = "image-meta";
pub const NOTES_START_SENTINEL: &str = "<!-- oxlide-notes-start -->";
pub const NOTES_END_SENTINEL: &str = "<!-- oxlide-notes-end -->";
pub const VISIBLE_START_SENTINEL: &str = "<!-- oxlide-visible-start -->";
pub const VISIBLE_END_SENTINEL: &str = "<!-- oxlide-visible-end -->";

const IMAGE_EXTENSIONS: &[&str] = &[".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BreakKind {
    Slide,
    Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub blank: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetEntry {
    pub rw_start: usize,
    pub rw_end: usize,
    pub orig_start: usize,
    pub orig_end: usize,
}

impl OffsetEntry {
    fn pure_insertion(rw_pos: usize, orig_pos: usize, len: usize) -> Self {
        Self {
            rw_start: rw_pos,
            rw_end: rw_pos + len,
            orig_start: orig_pos,
            orig_end: orig_pos,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepassOutput {
    pub rewritten: String,
    pub entries: Vec<OffsetEntry>,
    pub lines: Vec<Line>,
}

pub fn scan_lines(source: &str) -> Vec<Line> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && bytes[i] != b'\n' {
            i += 1;
        }
        let end = i;
        let blank = source[start..end].chars().all(char::is_whitespace);
        lines.push(Line { start, end, blank });
        if i < bytes.len() && bytes[i] == b'\n' {
            i += 1;
        }
    }
    lines
}

fn line_text(source: &str, line: Line) -> &str {
    &source[line.start..line.end]
}

fn detect_image_path(line: &str) -> Option<&str> {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    if trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    for ext in IMAGE_EXTENSIONS {
        if lower.ends_with(ext) {
            return Some(trimmed);
        }
    }
    None
}

fn parse_metadata_line(line: &str) -> Option<(&str, &str)> {
    if !line.starts_with([' ', '\t']) {
        return None;
    }
    let trimmed = line.trim_start();
    let colon = trimmed.find(':')?;
    let key = &trimmed[..colon];
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    let value = trimmed[colon + 1..].trim();
    if value.is_empty() {
        return None;
    }
    Some((key, value))
}

fn strip_outer_quotes(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn normalize_align(axis: &str, value: &str) -> Option<&'static str> {
    let v = value.to_ascii_lowercase();
    match (axis, v.as_str()) {
        ("x", "left") | ("x", "start") => Some("start"),
        ("x", "right") | ("x", "end") => Some("end"),
        ("x", "center") => Some("center"),
        ("y", "top") | ("y", "start") => Some("start"),
        ("y", "bottom") | ("y", "end") => Some("end"),
        ("y", "center") => Some("center"),
        _ => None,
    }
}

fn normalize_size(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "contain" => Some("contain"),
        "cover" => Some("cover"),
        "fit-width" => Some("fit-width"),
        "fit-height" => Some("fit-height"),
        _ => None,
    }
}

fn build_meta_json(entries: &[(String, String, SourceSpan)]) -> Result<String, ParseError> {
    let mut map = serde_json::Map::new();
    for (key, raw_value, span) in entries {
        let stripped = strip_outer_quotes(raw_value).to_string();
        let json_value = match key.as_str() {
            "size" => {
                let Some(s) = normalize_size(&stripped) else {
                    return Err(ParseError::InvalidImageMeta {
                        span: *span,
                        key: key.clone(),
                        value: stripped,
                    });
                };
                serde_json::Value::String(s.to_string())
            }
            "x" | "y" => {
                let Some(s) = normalize_align(key, &stripped) else {
                    return Err(ParseError::InvalidImageMeta {
                        span: *span,
                        key: key.clone(),
                        value: stripped,
                    });
                };
                serde_json::Value::String(s.to_string())
            }
            "background" => serde_json::Value::String(stripped),
            "opacity" => {
                let parsed: Result<f32, _> = stripped.parse();
                match parsed {
                    Ok(v) if (0.0..=1.0).contains(&v) => serde_json::Value::Number(
                        serde_json::Number::from_f64(v as f64).ok_or_else(|| {
                            ParseError::InvalidImageMeta {
                                span: *span,
                                key: key.clone(),
                                value: stripped.clone(),
                            }
                        })?,
                    ),
                    _ => {
                        return Err(ParseError::InvalidImageMeta {
                            span: *span,
                            key: key.clone(),
                            value: stripped,
                        });
                    }
                }
            }
            _ => continue,
        };
        map.insert(key.clone(), json_value);
    }
    if map.is_empty() {
        return Ok(String::new());
    }
    Ok(serde_json::Value::Object(map).to_string())
}

#[derive(Debug, Clone)]
struct ImageBlock {
    path_line: usize,
    meta_line_end: usize,
    path: String,
    meta_json: String,
}

fn detect_image_blocks(source: &str, lines: &[Line]) -> Result<Vec<ImageBlock>, ParseError> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let preceded_ok = i == 0 || lines[i - 1].blank;
        if !preceded_ok {
            i += 1;
            continue;
        }
        let text = line_text(source, lines[i]);
        let Some(path) = detect_image_path(text) else {
            i += 1;
            continue;
        };
        let mut meta = Vec::<(String, String, SourceSpan)>::new();
        let mut j = i + 1;
        while j < lines.len() {
            let next_text = line_text(source, lines[j]);
            if let Some((key, value)) = parse_metadata_line(next_text) {
                meta.push((
                    key.to_string(),
                    value.to_string(),
                    SourceSpan {
                        start: lines[j].start,
                        end: lines[j].end,
                    },
                ));
                j += 1;
            } else {
                break;
            }
        }
        let meta_json = build_meta_json(&meta)?;
        blocks.push(ImageBlock {
            path_line: i,
            meta_line_end: j,
            path: path.to_string(),
            meta_json,
        });
        i = j;
    }
    Ok(blocks)
}

fn is_fence_open(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn fence_marker_of(text: &str) -> &'static str {
    let t = text.trim_start();
    if t.starts_with("```") { "```" } else { "~~~" }
}

fn find_html_comment_end(source: &str, lines: &[Line], start: usize) -> usize {
    let first = line_text(source, lines[start]);
    if first.contains("-->") {
        return start + 1;
    }
    let mut j = start + 1;
    while j < lines.len() {
        if line_text(source, lines[j]).contains("-->") {
            return j + 1;
        }
        j += 1;
    }
    lines.len()
}

fn find_fence_end(source: &str, lines: &[Line], start: usize, marker: &str) -> usize {
    let mut j = start + 1;
    while j < lines.len() {
        let t = line_text(source, lines[j]);
        if t.trim_start().starts_with(marker) {
            return j + 1;
        }
        j += 1;
    }
    lines.len()
}

fn is_markdown_image_line(text: &str) -> bool {
    let t = text.trim_end();
    if !t.starts_with("![") {
        return false;
    }
    let Some(close_bracket) = t.find(']') else {
        return false;
    };
    let rest = &t[close_bracket + 1..];
    rest.starts_with('(') && rest.ends_with(')') && rest.len() >= 2
}

fn looks_like_list_item(text: &str) -> bool {
    let trimmed = text.trim_start_matches([' ']);
    let bytes = trimmed.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let c = bytes[0];
    if c == b'-' || c == b'*' || c == b'+' {
        return bytes.len() == 1 || bytes[1] == b' ' || bytes[1] == b'\t';
    }
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')') {
        let rest = i + 1;
        return rest == bytes.len() || bytes[rest] == b' ' || bytes[rest] == b'\t';
    }
    false
}

fn has_trailing_newline(source: &str, line: Line) -> bool {
    line.end < source.len() && source.as_bytes()[line.end] == b'\n'
}

fn emit_line_verbatim(source: &str, line: Line, rewritten: &mut String) {
    rewritten.push_str(&source[line.start..line.end]);
    if has_trailing_newline(source, line) {
        rewritten.push('\n');
    }
}

fn emit_image_block(
    source: &str,
    lines: &[Line],
    block: &ImageBlock,
    rewritten: &mut String,
    entries: &mut Vec<OffsetEntry>,
) {
    let line = lines[block.path_line];
    let last_line = lines[block.meta_line_end - 1];
    let trailing = has_trailing_newline(source, last_line);
    let orig_start = line.start;
    let orig_end = if trailing { last_line.end + 1 } else { last_line.end };

    let rw_start = rewritten.len();
    rewritten.push_str("![](");
    rewritten.push_str(&block.path);
    rewritten.push(')');
    rewritten.push('\n');
    if !block.meta_json.is_empty() {
        rewritten.push_str("<!-- oxlide-image-meta: ");
        rewritten.push_str(&block.meta_json);
        rewritten.push_str(" -->");
        if trailing {
            rewritten.push('\n');
        }
    } else if !trailing {
        rewritten.pop();
    }
    let rw_end = rewritten.len();
    entries.push(OffsetEntry {
        rw_start,
        rw_end,
        orig_start,
        orig_end,
    });
}

fn find_plain_block_end(
    source: &str,
    lines: &[Line],
    start: usize,
    image_block_at: &[Option<usize>],
) -> usize {
    let first_text = line_text(source, lines[start]);
    let first_tab = first_text.starts_with('\t');
    let first_content = if first_tab { &first_text[1..] } else { first_text };
    let first_is_list = looks_like_list_item(first_content);

    let mut j = start + 1;
    while j < lines.len() && !lines[j].blank {
        if image_block_at[j].is_some() {
            break;
        }
        let text = line_text(source, lines[j]);
        if text.starts_with('#') {
            break;
        }
        if is_fence_open(text) {
            break;
        }

        let this_tab = text.starts_with('\t');
        let this_content = if this_tab { &text[1..] } else { text };
        let this_is_list = looks_like_list_item(this_content);

        if first_is_list {
            if this_tab != first_tab {
                break;
            }
        } else if this_is_list {
            break;
        }

        j += 1;
    }
    j
}

fn emit_wrapped_block(
    source: &str,
    lines: &[Line],
    start: usize,
    end: usize,
    opener_text: &str,
    closer_text: &str,
    strip_leading_tab: bool,
    rewritten: &mut String,
    entries: &mut Vec<OffsetEntry>,
) {
    let block_orig_start = lines[start].start;
    let last = lines[end - 1];
    let trailing = has_trailing_newline(source, last);
    let block_orig_end = if trailing { last.end + 1 } else { last.end };

    let rw_pos = rewritten.len();
    let opener = format!("{}\n\n", opener_text);
    rewritten.push_str(&opener);
    entries.push(OffsetEntry::pure_insertion(
        rw_pos,
        block_orig_start,
        opener.len(),
    ));

    for i in start..end {
        let line = lines[i];
        let text = &source[line.start..line.end];
        if strip_leading_tab {
            if let Some(rest) = text.strip_prefix('\t') {
                let rw_tab_pos = rewritten.len();
                entries.push(OffsetEntry {
                    rw_start: rw_tab_pos,
                    rw_end: rw_tab_pos,
                    orig_start: line.start,
                    orig_end: line.start + 1,
                });
                rewritten.push_str(rest);
            } else {
                rewritten.push_str(text);
            }
        } else {
            rewritten.push_str(text);
        }
        if has_trailing_newline(source, line) {
            rewritten.push('\n');
        }
    }

    let rw_pos = rewritten.len();
    let closer = if trailing {
        format!("\n{}\n\n", closer_text)
    } else {
        format!("\n\n{}\n\n", closer_text)
    };
    rewritten.push_str(&closer);
    entries.push(OffsetEntry::pure_insertion(
        rw_pos,
        block_orig_end,
        closer.len(),
    ));
}

pub fn prepass(source: &str) -> Result<PrepassOutput, ParseError> {
    let lines = scan_lines(source);
    let image_blocks = detect_image_blocks(source, &lines)?;

    let mut image_block_at: Vec<Option<usize>> = vec![None; lines.len()];
    for (idx, b) in image_blocks.iter().enumerate() {
        image_block_at[b.path_line] = Some(idx);
    }

    let mut insert_after_line: Vec<Option<BreakKind>> = vec![None; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if lines[i].blank {
            let run_start = i;
            while i < lines.len() && lines[i].blank {
                i += 1;
            }
            let run_end = i;
            let has_before = run_start > 0 && !lines[run_start - 1].blank;
            let has_after = run_end < lines.len() && !lines[run_end].blank;
            if has_before && has_after {
                let kind = if (run_end - run_start) >= 2 {
                    BreakKind::Slide
                } else {
                    BreakKind::Cell
                };
                insert_after_line[run_start] = Some(kind);
            }
        } else {
            i += 1;
        }
    }

    let mut rewritten = String::with_capacity(source.len() + 256);
    let mut entries: Vec<OffsetEntry> = Vec::new();

    let mut idx = 0;
    while idx < lines.len() {
        if let Some(block_idx) = image_block_at[idx] {
            let block = &image_blocks[block_idx];
            emit_image_block(source, &lines, block, &mut rewritten, &mut entries);
            idx = block.meta_line_end;
            continue;
        }

        let line = lines[idx];

        if line.blank {
            emit_line_verbatim(source, line, &mut rewritten);
            if let Some(kind) = insert_after_line[idx] {
                let sentinel = match kind {
                    BreakKind::Slide => SLIDE_BREAK_SENTINEL,
                    BreakKind::Cell => CELL_BREAK_SENTINEL,
                };
                let rw_pos = rewritten.len();
                let injected = format!("{}\n\n", sentinel);
                let len = injected.len();
                rewritten.push_str(&injected);
                let orig_pos = if has_trailing_newline(source, line) {
                    line.end + 1
                } else {
                    line.end
                };
                entries.push(OffsetEntry::pure_insertion(rw_pos, orig_pos, len));
            }
            idx += 1;
            continue;
        }

        let text = line_text(source, line);

        if text.starts_with('#') {
            emit_line_verbatim(source, line, &mut rewritten);
            idx += 1;
            continue;
        }

        if is_fence_open(text) {
            let marker = fence_marker_of(text);
            let fence_end = find_fence_end(source, &lines, idx, marker);
            for i in idx..fence_end {
                emit_line_verbatim(source, lines[i], &mut rewritten);
            }
            idx = fence_end;
            continue;
        }

        if is_markdown_image_line(text) {
            emit_line_verbatim(source, line, &mut rewritten);
            idx += 1;
            continue;
        }

        if text.starts_with("<!--") {
            let comment_end = find_html_comment_end(source, &lines, idx);
            for i in idx..comment_end {
                emit_line_verbatim(source, lines[i], &mut rewritten);
            }
            idx = comment_end;
            continue;
        }

        let is_tab = text.starts_with('\t');
        let block_end = find_plain_block_end(source, &lines, idx, &image_block_at);

        if is_tab {
            emit_wrapped_block(
                source,
                &lines,
                idx,
                block_end,
                VISIBLE_START_SENTINEL,
                VISIBLE_END_SENTINEL,
                true,
                &mut rewritten,
                &mut entries,
            );
        } else {
            emit_wrapped_block(
                source,
                &lines,
                idx,
                block_end,
                NOTES_START_SENTINEL,
                NOTES_END_SENTINEL,
                false,
                &mut rewritten,
                &mut entries,
            );
        }
        idx = block_end;
    }

    Ok(PrepassOutput {
        rewritten,
        entries,
        lines,
    })
}

pub fn rewritten_to_original(rw: usize, entries: &[OffsetEntry]) -> usize {
    let mut adjust: isize = 0;
    for e in entries {
        if rw < e.rw_start {
            break;
        } else if rw < e.rw_end {
            return e.orig_start;
        } else {
            adjust = e.orig_end as isize - e.rw_end as isize;
        }
    }
    (rw as isize + adjust) as usize
}

pub fn entry_containing(rw: usize, entries: &[OffsetEntry]) -> Option<&OffsetEntry> {
    entries
        .iter()
        .find(|e| e.rw_start <= rw && rw < e.rw_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_lines_empty() {
        assert!(scan_lines("").is_empty());
    }

    #[test]
    fn scan_lines_single_line_no_newline() {
        let lines = scan_lines("hello");
        assert_eq!(
            lines,
            vec![Line {
                start: 0,
                end: 5,
                blank: false
            }]
        );
    }

    #[test]
    fn scan_lines_single_line_trailing_newline() {
        let lines = scan_lines("hello\n");
        assert_eq!(
            lines,
            vec![Line {
                start: 0,
                end: 5,
                blank: false
            }]
        );
    }

    #[test]
    fn scan_lines_blank_detection() {
        let lines = scan_lines("a\n\n   \nb");
        assert_eq!(lines.len(), 4);
        assert!(!lines[0].blank);
        assert!(lines[1].blank);
        assert!(lines[2].blank);
        assert!(!lines[3].blank);
    }

    #[test]
    fn scan_lines_offsets_round_trip() {
        let source = "first\nsecond\n\nthird";
        let lines = scan_lines(source);
        assert_eq!(&source[lines[0].start..lines[0].end], "first");
        assert_eq!(&source[lines[1].start..lines[1].end], "second");
        assert_eq!(&source[lines[2].start..lines[2].end], "");
        assert_eq!(&source[lines[3].start..lines[3].end], "third");
    }

    #[test]
    fn prepass_injects_cell_break_for_single_blank() {
        let source = "# A\n\n# B";
        let out = prepass(source).unwrap();
        assert!(out.rewritten.contains(CELL_BREAK_SENTINEL));
        assert!(!out.rewritten.contains(SLIDE_BREAK_SENTINEL));
        assert_eq!(out.entries.len(), 1);
    }

    #[test]
    fn prepass_injects_slide_break_for_double_blank() {
        let source = "# A\n\n\n\n# B";
        let out = prepass(source).unwrap();
        assert!(out.rewritten.contains(SLIDE_BREAK_SENTINEL));
        assert!(!out.rewritten.contains(CELL_BREAK_SENTINEL));
        assert_eq!(out.entries.len(), 1);
    }

    #[test]
    fn prepass_leading_blanks_not_injected() {
        let source = "\n\n\n\nhello";
        let out = prepass(source).unwrap();
        assert!(!out.rewritten.contains(SLIDE_BREAK_SENTINEL));
        assert!(!out.rewritten.contains(CELL_BREAK_SENTINEL));
    }

    #[test]
    fn prepass_trailing_blanks_not_injected() {
        let source = "hello\n\n\n\n";
        let out = prepass(source).unwrap();
        assert!(!out.rewritten.contains(SLIDE_BREAK_SENTINEL));
        assert!(!out.rewritten.contains(CELL_BREAK_SENTINEL));
    }

    #[test]
    fn prepass_no_blank_line_no_break_injected() {
        let source = "# A\n\tprose";
        let out = prepass(source).unwrap();
        assert!(!out.rewritten.contains(CELL_BREAK_SENTINEL));
        assert!(!out.rewritten.contains(SLIDE_BREAK_SENTINEL));
    }

    #[test]
    fn rewritten_to_original_identity_without_entries() {
        assert_eq!(rewritten_to_original(10, &[]), 10);
    }

    #[test]
    fn rewritten_to_original_subtracts_prior_insertions() {
        let entries = vec![OffsetEntry::pure_insertion(5, 5, 10)];
        assert_eq!(rewritten_to_original(4, &entries), 4);
        assert_eq!(rewritten_to_original(15, &entries), 5);
        assert_eq!(rewritten_to_original(20, &entries), 10);
    }

    #[test]
    fn rewritten_to_original_clamps_inside_insertion() {
        let entries = vec![OffsetEntry::pure_insertion(5, 5, 10)];
        assert_eq!(rewritten_to_original(10, &entries), 5);
    }

    #[test]
    fn rewritten_to_original_replacement_past_end_uses_orig_end() {
        let entries = vec![OffsetEntry {
            rw_start: 0,
            rw_end: 13,
            orig_start: 0,
            orig_end: 8,
        }];
        assert_eq!(rewritten_to_original(13, &entries), 8);
        assert_eq!(rewritten_to_original(14, &entries), 9);
    }

    #[test]
    fn detect_image_path_accepts_png() {
        assert_eq!(detect_image_path("hero.png"), Some("hero.png"));
    }

    #[test]
    fn detect_image_path_case_insensitive() {
        assert_eq!(detect_image_path("HERO.PNG"), Some("HERO.PNG"));
    }

    #[test]
    fn detect_image_path_rejects_indented() {
        assert_eq!(detect_image_path("  hero.png"), None);
    }

    #[test]
    fn detect_image_path_rejects_with_spaces() {
        assert_eq!(detect_image_path("hero image.png"), None);
    }

    #[test]
    fn detect_image_path_rejects_non_image_ext() {
        assert_eq!(detect_image_path("hero.txt"), None);
    }

    #[test]
    fn parse_metadata_line_strips_indent_and_trims_value() {
        assert_eq!(
            parse_metadata_line("\tsize: contain"),
            Some(("size", "contain"))
        );
        assert_eq!(
            parse_metadata_line("    x: right"),
            Some(("x", "right"))
        );
    }

    #[test]
    fn parse_metadata_line_rejects_no_indent() {
        assert!(parse_metadata_line("size: contain").is_none());
    }

    #[test]
    fn parse_metadata_line_rejects_empty_value() {
        assert!(parse_metadata_line("    size:").is_none());
        assert!(parse_metadata_line("    size:   ").is_none());
    }

    #[test]
    fn strip_outer_quotes_double() {
        assert_eq!(strip_outer_quotes("\"#fff\""), "#fff");
    }

    #[test]
    fn strip_outer_quotes_single() {
        assert_eq!(strip_outer_quotes("'value'"), "value");
    }

    #[test]
    fn strip_outer_quotes_none() {
        assert_eq!(strip_outer_quotes("#fff"), "#fff");
    }

    #[test]
    fn prepass_wraps_notes_block() {
        let out = prepass("notes line\n").unwrap();
        assert!(out.rewritten.contains(NOTES_START_SENTINEL));
        assert!(out.rewritten.contains(NOTES_END_SENTINEL));
    }

    #[test]
    fn prepass_wraps_tab_visible_block_and_strips_tab() {
        let out = prepass("\tvisible\n").unwrap();
        assert!(out.rewritten.contains(VISIBLE_START_SENTINEL));
        assert!(out.rewritten.contains(VISIBLE_END_SENTINEL));
        assert!(out.rewritten.contains("visible"));
        assert!(!out.rewritten.contains("\tvisible"));
    }

    #[test]
    fn prepass_heading_not_wrapped() {
        let out = prepass("# Title\n").unwrap();
        assert!(!out.rewritten.contains(NOTES_START_SENTINEL));
        assert!(!out.rewritten.contains(VISIBLE_START_SENTINEL));
    }

    #[test]
    fn prepass_fenced_code_not_wrapped_even_if_content_unindented() {
        let out = prepass("```\nhello\n```\n").unwrap();
        assert!(!out.rewritten.contains(NOTES_START_SENTINEL));
        assert!(!out.rewritten.contains(VISIBLE_START_SENTINEL));
    }

    #[test]
    fn prepass_tab_visible_span_maps_back_through_tab_deletion() {
        let source = "\tHello\n";
        let out = prepass(source).unwrap();
        let hello_rw = out.rewritten.find("Hello").expect("Hello should appear");
        let orig_pos = rewritten_to_original(hello_rw, &out.entries);
        assert_eq!(orig_pos, 1, "rewritten 'H' should map to original offset 1 (after tab)");
    }

    #[test]
    fn looks_like_list_item_unordered() {
        assert!(looks_like_list_item("- hello"));
        assert!(looks_like_list_item("* hello"));
        assert!(looks_like_list_item("+ hello"));
        assert!(!looks_like_list_item("-hello"));
        assert!(!looks_like_list_item("text - word"));
    }

    #[test]
    fn looks_like_list_item_ordered() {
        assert!(looks_like_list_item("1. hello"));
        assert!(looks_like_list_item("42. hello"));
        assert!(looks_like_list_item("1) hello"));
        assert!(!looks_like_list_item("1hello"));
        assert!(!looks_like_list_item("1.hello"));
    }

    #[test]
    fn is_markdown_image_line_valid() {
        assert!(is_markdown_image_line("![alt](path.png)"));
        assert!(is_markdown_image_line("![](path.png)"));
    }

    #[test]
    fn is_markdown_image_line_rejects_inline() {
        assert!(!is_markdown_image_line("text ![img](p.png) more"));
        assert!(!is_markdown_image_line("![img](p.png) trailing"));
    }
}
