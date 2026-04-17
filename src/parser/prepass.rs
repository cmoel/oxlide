pub const SLIDE_BREAK_SENTINEL: &str = "<!-- oxlide-slide-break -->";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub blank: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepassOutput {
    pub rewritten: String,
    pub insertions: Vec<(usize, usize)>,
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

pub fn prepass(source: &str) -> PrepassOutput {
    let lines = scan_lines(source);
    let mut insert_after_line: Vec<bool> = vec![false; lines.len()];

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
            if has_before && has_after && (run_end - run_start) >= 2 {
                insert_after_line[run_start] = true;
            }
        } else {
            i += 1;
        }
    }

    let mut rewritten = String::with_capacity(source.len() + 64);
    let mut insertions: Vec<(usize, usize)> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        rewritten.push_str(&source[line.start..line.end]);
        let has_newline_after = line.end < source.len() && source.as_bytes()[line.end] == b'\n';
        if has_newline_after {
            rewritten.push('\n');
        }
        if insert_after_line[idx] {
            let rw_pos = rewritten.len();
            let injected = format!("{}\n\n", SLIDE_BREAK_SENTINEL);
            rewritten.push_str(&injected);
            insertions.push((rw_pos, injected.len()));
        }
    }

    PrepassOutput {
        rewritten,
        insertions,
        lines,
    }
}

pub fn rewritten_to_original(rw: usize, insertions: &[(usize, usize)]) -> usize {
    let mut shift = 0;
    for (pos, len) in insertions {
        if pos + len <= rw {
            shift += len;
        } else if *pos <= rw {
            return pos.saturating_sub(shift);
        } else {
            break;
        }
    }
    rw - shift
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
    fn prepass_no_double_blank_is_identity() {
        let source = "# A\n\n# B";
        let out = prepass(source);
        assert_eq!(out.rewritten, source);
        assert!(out.insertions.is_empty());
    }

    #[test]
    fn prepass_injects_sentinel_for_double_blank() {
        let source = "# A\n\n\n\n# B";
        let out = prepass(source);
        assert!(out.rewritten.contains(SLIDE_BREAK_SENTINEL));
        assert_eq!(out.insertions.len(), 1);
    }

    #[test]
    fn prepass_leading_blanks_not_injected() {
        let source = "\n\n\n\nhello";
        let out = prepass(source);
        assert!(out.insertions.is_empty());
    }

    #[test]
    fn prepass_trailing_blanks_not_injected() {
        let source = "hello\n\n\n\n";
        let out = prepass(source);
        assert!(out.insertions.is_empty());
    }

    #[test]
    fn rewritten_to_original_identity_without_insertions() {
        assert_eq!(rewritten_to_original(10, &[]), 10);
    }

    #[test]
    fn rewritten_to_original_subtracts_prior_insertions() {
        let insertions = vec![(5, 10)];
        assert_eq!(rewritten_to_original(4, &insertions), 4);
        assert_eq!(rewritten_to_original(15, &insertions), 5);
        assert_eq!(rewritten_to_original(20, &insertions), 10);
    }

    #[test]
    fn rewritten_to_original_clamps_inside_insertion() {
        let insertions = vec![(5, 10)];
        assert_eq!(rewritten_to_original(10, &insertions), 5);
    }
}
