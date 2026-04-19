use std::fs;
use std::path::{Path, PathBuf};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use oxlide::parser::{Block, InlineSpan, SlideDeck};
use oxlide::render::{Theme, compute_inner_area, render_slide};

const CANVAS_WIDTH: u16 = 120;
const CANVAS_HEIGHT: u16 = 40;

fn discover_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", dir.display(), e))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    paths.sort();
    paths
}

fn buffer_text(buf: &Buffer) -> String {
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            s.push_str(buf[(x, y)].symbol());
        }
        s.push('\n');
    }
    s
}

fn has_non_space_cell(buf: &Buffer) -> bool {
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if !buf[(x, y)].symbol().trim().is_empty() {
                return true;
            }
        }
    }
    false
}

fn collect_visible_words(deck: &SlideDeck) -> Vec<String> {
    let mut out = Vec::new();
    for slide in &deck.slides {
        for cell in &slide.cells {
            collect_block_words(&cell.blocks, &mut out);
        }
    }
    out
}

fn collect_block_words(blocks: &[Block], out: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Heading { spans, .. } | Block::Paragraph { spans, .. } => {
                collect_span_words(spans, out);
            }
            Block::List { items, .. } => {
                for item in items {
                    collect_block_words(&item.blocks, out);
                }
            }
            Block::Image { alt, src, .. } => {
                push_words(alt, out);
                push_words(src, out);
            }
            Block::CodeBlock { source, .. } => {
                push_words(source, out);
            }
        }
    }
}

fn collect_span_words(spans: &[InlineSpan], out: &mut Vec<String>) {
    for span in spans {
        match span {
            InlineSpan::Text(t) | InlineSpan::Code(t) => push_words(t, out),
            InlineSpan::Strong(c) | InlineSpan::Emphasis(c) => collect_span_words(c, out),
            InlineSpan::Link { text, .. } => collect_span_words(text, out),
            InlineSpan::Image { alt, .. } => push_words(alt, out),
        }
    }
}

fn push_words(text: &str, out: &mut Vec<String>) {
    for word in text.split_whitespace() {
        let cleaned: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
        if cleaned.chars().count() >= 4 {
            out.push(cleaned);
        }
    }
}

#[test]
fn every_fixture_renders() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/decks");
    assert!(
        dir.is_dir(),
        "fixture directory not found: {}",
        dir.display()
    );

    let fixtures = discover_fixtures(&dir);
    assert!(
        !fixtures.is_empty(),
        "no fixtures discovered in {}",
        dir.display()
    );

    let theme = Theme::default();
    let area = Rect::new(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);

    let mut failures: Vec<String> = Vec::new();
    for path in &fixtures {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        let source = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{}: failed to read: {}", name, e));

        let deck = match oxlide::parse_deck(&source) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{}: parse failed: {}", name, e));
                continue;
            }
        };

        if deck.slides.is_empty() {
            failures.push(format!("{}: parsed deck has zero slides", name));
            continue;
        }

        let mut any_non_empty = false;
        let mut combined = String::new();
        for slide in &deck.slides {
            let mut buf = Buffer::empty(area);
            render_slide(slide, area, &mut buf, &theme);
            if has_non_space_cell(&buf) {
                any_non_empty = true;
            }
            combined.push_str(&buffer_text(&buf));
        }

        if !any_non_empty {
            failures.push(format!("{}: all slides rendered empty buffers", name));
            continue;
        }

        let words = collect_visible_words(&deck);
        if !words.is_empty() {
            let found = words.iter().any(|w| combined.contains(w.as_str()));
            if !found {
                let sample: Vec<&str> =
                    words.iter().take(5).map(String::as_str).collect();
                failures.push(format!(
                    "{}: no visible word from the source appeared in any rendered buffer; tried {:?}",
                    name, sample
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "render fixtures failed:\n  {}",
        failures.join("\n  ")
    );
}

// -------------------------------------------------------------------------
// Composition acceptance tests (oxlide-92p).
// -------------------------------------------------------------------------

fn render_to_buffer(source: &str, area: Rect) -> Buffer {
    let deck = oxlide::parse_deck(source).expect("fixture parses");
    let theme = Theme::default();
    let mut buf = Buffer::empty(area);
    let slide = deck.slides.first().expect("at least one slide");
    render_slide(slide, area, &mut buf, &theme);
    buf
}

fn row_string(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

fn find_row(buf: &Buffer, needle: &str) -> Option<(u16, String)> {
    for y in 0..buf.area.height {
        let row = row_string(buf, y);
        if row.contains(needle) {
            return Some((y, row));
        }
    }
    None
}

#[test]
fn padding_at_120_cols_matches_acceptance_range() {
    // Acceptance: 120 cols → content x-range is [9..=110].
    let area = Rect::new(0, 0, 120, 40);
    let (inner, _) = compute_inner_area(area, &Theme::default());
    assert_eq!(inner.x, 9);
    assert_eq!(inner.x + inner.width - 1, 110);
}

#[test]
fn chrome_rows_zero_is_a_noop() {
    let area = Rect::new(0, 0, 120, 40);
    let theme = Theme::default();
    assert_eq!(theme.chrome_rows, 0, "default theme reserves no chrome");
    let (inner, chrome) = compute_inner_area(area, &theme);
    assert_eq!(inner.height, 40);
    assert_eq!(chrome.height, 0);
}

#[test]
fn hero_h1_only_centers_on_80_by_24() {
    // Acceptance/snapshot: '# Single H1' on 80×24 → H1 within rows 11±2, centered horizontally.
    let source = "# Single H1\n";
    let area = Rect::new(0, 0, 80, 24);
    let buf = render_to_buffer(source, area);

    let (y, row) = find_row(&buf, "Single H1").expect("heading rendered");
    assert!(
        (9..=13).contains(&y),
        "heading row was {}, expected 11±2",
        y
    );

    let start = row.find("Single H1").expect("needle in row");
    // Horizontally centered: symmetric padding on both sides of "Single H1".
    let text_width = "Single H1".width();
    let left_pad = start;
    let right_pad = 80 - (start + text_width);
    assert!(
        left_pad.abs_diff(right_pad) <= 1,
        "horizontal centering should be symmetric; left={}, right={}",
        left_pad,
        right_pad
    );
}

#[test]
fn hero_h1_only_centers_on_120_by_40() {
    // Acceptance: 120×40 terminal + single-H1 slide → heading centered ± 2 rows.
    let source = "# Hero\n";
    let area = Rect::new(0, 0, 120, 40);
    let buf = render_to_buffer(source, area);

    let (y, _) = find_row(&buf, "Hero").expect("heading rendered");
    // inner.height = 40, content height = 1, top_offset = 19. y ≈ 19 ± 2.
    assert!(
        (17..=21).contains(&y),
        "heading row was {}, expected center ± 2",
        y
    );
}

#[test]
fn multi_block_cell_heading_is_anchored_and_padded() {
    // Acceptance: non-hero slide (heading + bullets in one cell) — heading at
    // the top of the padded inner area (not at (0,0)) with a spacer row before
    // the body.
    let source = "# Anchored\n\t- One\n\t- Two\n";
    let area = Rect::new(0, 0, 120, 40);
    let buf = render_to_buffer(source, area);

    let (y, row) = find_row(&buf, "Anchored").expect("heading rendered");
    let x = row.find("Anchored").unwrap();
    // Heading is anchored near the top of inner area (not vertically centered).
    assert!(y <= 2, "heading should be at top, got row {}", y);
    // Heading is padded — 120-col padding is 9 cells.
    assert_eq!(x, 9, "heading should start at x=9 (padded), got {}", x);

    // Bullets render below the heading, separated by a spacer row.
    let (bullet_y, _) = find_row(&buf, "• One").expect("first bullet rendered");
    assert!(
        bullet_y >= y + 2,
        "bullets ({}) should appear with a spacer after heading ({})",
        bullet_y,
        y
    );
}

#[test]
fn multi_cell_slide_places_first_cell_at_padded_origin() {
    // When a slide has separate cells (heading cell + list cell), the layout
    // engine may split horizontally. Either way, the first cell's origin must
    // respect composition padding and never be at (0,0).
    let source = "# Anchored\n\n\t- One\n\t- Two\n";
    let area = Rect::new(0, 0, 120, 40);
    let buf = render_to_buffer(source, area);

    let (y, row) = find_row(&buf, "Anchored").expect("heading rendered");
    let x = row.find("Anchored").unwrap();
    assert_eq!(x, 9, "heading should start at x=9 (padded), got {}", x);
    assert!(y <= 2, "heading should be at top of inner area, got {}", y);
}

#[test]
fn emoji_heading_horizontally_centered_at_40_80_120() {
    let source = "# 🎉 Party\n";
    // "🎉 Party" width = 2 + 1 + 5 = 8 cells.
    let expected_width = "🎉 Party".width();

    for cols in [40u16, 80, 120] {
        let area = Rect::new(0, 0, cols, 24);
        let buf = render_to_buffer(source, area);

        // The emoji must appear in the rendered buffer.
        let (_, row) = find_row(&buf, "🎉").expect("emoji in buffer");
        let start = row.find("🎉").unwrap();

        let (inner, _) = compute_inner_area(area, &Theme::default());
        let left_offset = start as u16 - inner.x;
        let trailing = inner.width - left_offset - expected_width as u16;
        // Centered within the inner area. Off-by-one tolerated for odd splits.
        assert!(
            left_offset.abs_diff(trailing) <= 1,
            "emoji heading not centered at {} cols; left_offset={}, trailing={}, inner={:?}",
            cols,
            left_offset,
            trailing,
            inner
        );
    }
}

#[test]
fn narrow_terminal_15x8_does_not_panic_and_renders_something() {
    let source = "# Title\n";
    let area = Rect::new(0, 0, 15, 8);
    let buf = render_to_buffer(source, area);
    // At least one visible cell — no panic, content shows up best-effort.
    let any_visible = (0..buf.area.height).any(|y| {
        let row = row_string(&buf, y);
        !row.trim().is_empty()
    });
    assert!(any_visible, "narrow terminal rendered nothing");
}

#[test]
fn short_terminal_does_not_vertically_center() {
    // rows < 5 → skip vertical centering, anchor at top.
    let source = "# Hi\n";
    let area = Rect::new(0, 0, 40, 4);
    let buf = render_to_buffer(source, area);
    let (y, _) = find_row(&buf, "Hi").expect("heading in short terminal");
    assert_eq!(y, 0, "short terminal should anchor heading at top");
}

#[test]
fn same_slide_rerendered_at_different_sizes_stays_correct() {
    // Acceptance: rendering the same slide successively at 40×20 and 80×40
    // produces correctly-centered output in both — no stale cached values.
    let source = "# Resize\n";
    let deck = oxlide::parse_deck(source).unwrap();
    let slide = &deck.slides[0];
    let theme = Theme::default();

    let a1 = Rect::new(0, 0, 40, 20);
    let mut buf1 = Buffer::empty(a1);
    render_slide(slide, a1, &mut buf1, &theme);

    let a2 = Rect::new(0, 0, 80, 40);
    let mut buf2 = Buffer::empty(a2);
    render_slide(slide, a2, &mut buf2, &theme);

    let (y1, row1) = find_row(&buf1, "Resize").expect("40x20 heading");
    let (y2, row2) = find_row(&buf2, "Resize").expect("80x40 heading");

    // Row 1: 40x20 → top_offset = (20-1)/2 = 9.
    assert!((8..=10).contains(&y1), "40x20 heading at row {}", y1);
    // Row 2: 80x40 → top_offset = (40-1)/2 = 19.
    assert!((18..=20).contains(&y2), "80x40 heading at row {}", y2);

    // Horizontal centering differs per width — both must be symmetric.
    let text_w = "Resize".width();
    let x1 = row1.find("Resize").unwrap();
    let x2 = row2.find("Resize").unwrap();
    assert!(
        (x1 as i64 - (40 - x1 as u16 - text_w as u16) as i64).abs() <= 1,
        "40x20 heading not centered"
    );
    assert!(
        (x2 as i64 - (80 - x2 as u16 - text_w as u16) as i64).abs() <= 1,
        "80x40 heading not centered"
    );
}

#[test]
fn render_at_40x20_after_80x40_is_independent() {
    // Render order reversed — confirms no cached dimension leaks between calls.
    let source = "# Sticky\n";
    let deck = oxlide::parse_deck(source).unwrap();
    let slide = &deck.slides[0];
    let theme = Theme::default();

    let big = Rect::new(0, 0, 80, 40);
    let mut big_buf = Buffer::empty(big);
    render_slide(slide, big, &mut big_buf, &theme);

    let small = Rect::new(0, 0, 40, 20);
    let mut small_buf = Buffer::empty(small);
    render_slide(slide, small, &mut small_buf, &theme);

    let (y, _) = find_row(&small_buf, "Sticky").expect("small heading");
    assert!((8..=10).contains(&y), "heading row in small buf was {}", y);
}
