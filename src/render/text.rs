use unicode_width::UnicodeWidthChar;

/// Truncate `input` so its rendered cell width does not exceed `max_cells`.
/// When truncation occurs the result ends with `…` (which is 1 cell wide), and
/// the returned string's total width is `≤ max_cells`. Wide graphemes (emoji,
/// CJK) are never split mid-codepoint; zero-width combining marks are kept
/// with their preceding base character.
pub fn truncate_to_width(input: &str, max_cells: usize) -> String {
    if max_cells == 0 {
        return String::new();
    }
    let total_width: usize = input
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
        .sum();
    if total_width <= max_cells {
        return input.to_string();
    }

    // Need at least one cell for the ellipsis. If there's no room for even
    // that plus one visible character, return just the ellipsis (1 cell).
    const ELLIPSIS: char = '…';
    let ellipsis_width = UnicodeWidthChar::width(ELLIPSIS).unwrap_or(1);
    if max_cells <= ellipsis_width {
        return ELLIPSIS.to_string();
    }
    let budget = max_cells - ellipsis_width;

    let mut out = String::new();
    let mut used: usize = 0;
    for c in input.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if w == 0 {
            // Combining mark / variation selector: attach to previous base
            // char only if we've already started emitting. Emitting a leading
            // combining mark without a base is meaningless.
            if !out.is_empty() {
                out.push(c);
            }
            continue;
        }
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push(ELLIPSIS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn width(s: &str) -> usize {
        UnicodeWidthStr::width(s)
    }

    #[test]
    fn short_ascii_returns_unchanged() {
        let out = truncate_to_width("hello", 10);
        assert_eq!(out, "hello");
        assert!(width(&out) <= 10);
    }

    #[test]
    fn ascii_at_exact_width_returns_unchanged() {
        let out = truncate_to_width("hello", 5);
        assert_eq!(out, "hello");
    }

    #[test]
    fn long_ascii_truncates_with_ellipsis() {
        let out = truncate_to_width("abcdefghij", 5);
        assert_eq!(out, "abcd…");
        assert!(width(&out) <= 5);
    }

    #[test]
    fn url_truncates_like_acceptance_criteria() {
        let out = truncate_to_width("https://github.com/cmoel/oxlide", 16);
        assert!(out.ends_with('…'), "expected ellipsis, got {:?}", out);
        assert!(
            width(&out) <= 16,
            "width {} exceeded 16 for {:?}",
            width(&out),
            out
        );
        assert!(out.starts_with("https://github."), "got {:?}", out);
    }

    #[test]
    fn cjk_characters_count_two_cells_each() {
        // "例えばabc" — 例 (2) + え (2) + ば (2) + a (1) + b (1) + c (1) = 9 cells.
        let input = "例えばabc";
        assert_eq!(width(input), 9);

        // Exactly fits: no truncation.
        let out = truncate_to_width(input, 9);
        assert_eq!(out, input);

        // Truncate with budget 6 → ellipsis budget = 5 cells. 例 (2) + え (2) =
        // 4 cells used; adding ば (2) would exceed; stop. Result: "例え…" (5).
        let out = truncate_to_width(input, 6);
        assert_eq!(out, "例え…");
        assert!(width(&out) <= 6);
    }

    #[test]
    fn cjk_never_splits_mid_codepoint() {
        // Budget is tight — we must not partially emit a wide char.
        let out = truncate_to_width("例えばabc", 4);
        // ellipsis_width=1, budget=3. 例 (2) used=2. え (2) would exceed. a (1)?
        // Greedy-up-to-budget emits what fits. We stop at the first char that
        // overflows — this preserves ordering. So: out = "例…" (width 3).
        assert_eq!(out, "例…");
        assert!(width(&out) <= 4);
    }

    #[test]
    fn emoji_single_codepoint_counts_as_two_cells() {
        // 😀 is 2 cells wide.
        let out = truncate_to_width("😀hello", 8);
        assert_eq!(out, "😀hello");
        assert!(width(&out) <= 8);
    }

    #[test]
    fn emoji_zwj_sequence_truncates_cleanly() {
        // 👨‍💻 is a ZWJ sequence (man + ZWJ + laptop). Rendered width varies by
        // terminal but unicode-width reports width for each codepoint; the
        // guarantee we enforce is: no mid-codepoint split and final width ≤ budget.
        let input = "👨‍💻 coder";
        let out = truncate_to_width(input, 6);
        assert!(width(&out) <= 6, "width {} for {:?}", width(&out), out);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn combining_marks_stay_with_base() {
        // "e\u{0301}" (e + combining acute) renders as 1 cell.
        let input = "e\u{0301}llo";
        assert_eq!(width(input), 4);
        let out = truncate_to_width(input, 4);
        assert_eq!(out, input);
    }

    #[test]
    fn zero_budget_returns_empty() {
        let out = truncate_to_width("hello", 0);
        assert_eq!(out, "");
    }

    #[test]
    fn one_cell_budget_returns_just_ellipsis() {
        // With 1 cell and a string that overflows, we can't fit ellipsis + a
        // base char — return just the ellipsis.
        let out = truncate_to_width("hello", 1);
        assert_eq!(out, "…");
        assert!(width(&out) <= 1);
    }

    #[test]
    fn ellipsis_width_boundary_emits_one_char_plus_ellipsis() {
        // Budget 2: 1 cell for a char, 1 cell for ellipsis.
        let out = truncate_to_width("hello", 2);
        assert_eq!(out, "h…");
        assert!(width(&out) <= 2);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert_eq!(truncate_to_width("", 5), "");
        assert_eq!(truncate_to_width("", 0), "");
    }

    #[test]
    fn width_invariant_holds_for_mixed_content() {
        let cases = [
            "https://例え.jp/path/with/中文",
            "short",
            "😀😀😀😀",
            "abc 例 def",
        ];
        for max in [1, 2, 3, 5, 8, 12, 40] {
            for case in &cases {
                let out = truncate_to_width(case, max);
                assert!(
                    width(&out) <= max,
                    "case={:?} max={} out={:?} width={}",
                    case,
                    max,
                    out,
                    width(&out)
                );
            }
        }
    }
}
