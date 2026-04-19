use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::{Paragraph, Widget};

use crate::parser::{Block, Slide};
use crate::render::engine::inline_to_line;
use crate::render::theme::Theme;

/// Narrow-terminal fallback threshold: below this width we skip horizontal
/// padding and let content claim every cell.
const NARROW_WIDTH_THRESHOLD: u16 = 20;

/// Short-terminal fallback threshold: below this height we skip vertical
/// centering for hero slides and anchor content at the top of the inner area.
const SHORT_HEIGHT_THRESHOLD: u16 = 5;

/// Split `outer` into the inner content area (padded on the sides, with chrome
/// reserved from the bottom) and the chrome area itself. Takes `Rect` fresh
/// from the caller every frame — no caching.
pub fn compute_inner_area(outer: Rect, theme: &Theme) -> (Rect, Rect) {
    let pad_x = if outer.width < NARROW_WIDTH_THRESHOLD {
        0
    } else {
        let raw = (outer.width as u32).saturating_mul(8) / 100;
        (raw as u16).clamp(3, 12)
    };

    let max_chrome = outer.height / 3;
    let chrome_rows = theme.chrome_rows.min(max_chrome);

    let inner_width = outer.width.saturating_sub(pad_x.saturating_mul(2));
    let inner_height = outer.height.saturating_sub(chrome_rows);

    let inner = Rect {
        x: outer.x + pad_x,
        y: outer.y,
        width: inner_width,
        height: inner_height,
    };
    let chrome = Rect {
        x: outer.x,
        y: outer.y + inner_height,
        width: outer.width,
        height: chrome_rows,
    };
    (inner, chrome)
}

/// A hero slide has a single cell whose blocks are either `[H1]` or
/// `[H1, Paragraph]`. Hero slides render vertically centered with the heading
/// horizontally centered. Every other shape — H1+list, H1+code, H1+image,
/// multi-cell slides — is anchored at the top of the inner area.
pub fn is_hero_slide(slide: &Slide) -> bool {
    if slide.cells.len() != 1 {
        return false;
    }
    let blocks = slide.cells[0].blocks.as_slice();
    matches!(
        blocks,
        [Block::Heading { level: 1, .. }]
            | [Block::Heading { level: 1, .. }, Block::Paragraph { .. }]
    )
}

pub(crate) fn render_hero(slide: &Slide, inner: Rect, buf: &mut Buffer, theme: &Theme) {
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let cell = match slide.cells.first() {
        Some(c) => c,
        None => return,
    };
    let has_subtitle = cell.blocks.len() == 2;
    let content_height: u16 = if has_subtitle { 3 } else { 1 };

    let top_offset = if inner.height < SHORT_HEIGHT_THRESHOLD {
        0
    } else {
        inner.height.saturating_sub(content_height) / 2
    };

    let heading_y = inner.y + top_offset;
    if heading_y >= inner.y + inner.height {
        return;
    }
    let heading_rect = Rect {
        x: inner.x,
        y: heading_y,
        width: inner.width,
        height: 1,
    };
    if let Block::Heading { spans, .. } = &cell.blocks[0] {
        let line = inline_to_line(spans, theme);
        Paragraph::new(line)
            .style(theme.heading[0])
            .alignment(Alignment::Center)
            .render(heading_rect, buf);
    }

    if has_subtitle {
        let subtitle_y = heading_y + 2;
        if subtitle_y < inner.y + inner.height {
            let sub_rect = Rect {
                x: inner.x,
                y: subtitle_y,
                width: inner.width,
                height: 1,
            };
            if let Block::Paragraph { spans, .. } = &cell.blocks[1] {
                let line = inline_to_line(spans, theme);
                Paragraph::new(line)
                    .style(theme.prose)
                    .alignment(Alignment::Center)
                    .render(sub_rect, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Cell, InlineSpan, ListItem, Slide, SourceSpan};

    fn src_span() -> SourceSpan {
        SourceSpan { start: 0, end: 0 }
    }

    fn heading_block(level: u8, text: &str) -> Block {
        Block::Heading {
            level,
            spans: vec![InlineSpan::Text(text.into())],
            span: src_span(),
        }
    }

    fn paragraph_block(text: &str) -> Block {
        Block::Paragraph {
            spans: vec![InlineSpan::Text(text.into())],
            span: src_span(),
        }
    }

    fn list_block() -> Block {
        Block::List {
            ordered: false,
            items: vec![ListItem {
                blocks: vec![paragraph_block("item")],
                span: src_span(),
            }],
            span: src_span(),
        }
    }

    fn code_block() -> Block {
        Block::CodeBlock {
            lang: None,
            source: "x".into(),
            span: src_span(),
        }
    }

    fn image_block() -> Block {
        Block::Image {
            src: "x.png".into(),
            alt: String::new(),
            meta: None,
            span: src_span(),
        }
    }

    fn cell_of(blocks: Vec<Block>) -> Cell {
        Cell {
            blocks,
            directives: vec![],
            span: src_span(),
        }
    }

    fn slide_of(cells: Vec<Cell>) -> Slide {
        Slide {
            cells,
            notes: vec![],
            directives: vec![],
            span: src_span(),
        }
    }

    // ----- compute_inner_area -----

    #[test]
    fn padding_at_120_is_8_percent_clamped() {
        let theme = Theme::paper_white();
        let outer = Rect::new(0, 0, 120, 40);
        let (inner, chrome) = compute_inner_area(outer, &theme);
        // 120 * 8 / 100 = 9, clamp(3,12) = 9. content x range = [9..=110].
        assert_eq!(inner.x, 9);
        assert_eq!(inner.width, 102);
        assert_eq!(inner.x + inner.width - 1, 110);
        assert_eq!(inner.y, 0);
        assert_eq!(inner.height, 40);
        assert_eq!(chrome.height, 0);
    }

    #[test]
    fn padding_clamp_low_at_40() {
        let theme = Theme::paper_white();
        // 40 * 8 / 100 = 3, clamp(3,12) = 3.
        let (inner, _) = compute_inner_area(Rect::new(0, 0, 40, 10), &theme);
        assert_eq!(inner.x, 3);
        assert_eq!(inner.width, 34);
    }

    #[test]
    fn padding_at_80_cols() {
        let theme = Theme::paper_white();
        // 80 * 8 / 100 = 6, clamp(3,12) = 6.
        let (inner, _) = compute_inner_area(Rect::new(0, 0, 80, 24), &theme);
        assert_eq!(inner.x, 6);
        assert_eq!(inner.width, 68);
    }

    #[test]
    fn padding_clamp_high_at_200() {
        let theme = Theme::paper_white();
        // 200 * 8 / 100 = 16, clamp(3,12) = 12.
        let (inner, _) = compute_inner_area(Rect::new(0, 0, 200, 40), &theme);
        assert_eq!(inner.x, 12);
        assert_eq!(inner.width, 176);
    }

    #[test]
    fn narrow_fallback_disables_padding_below_20() {
        let theme = Theme::paper_white();
        let (inner, _) = compute_inner_area(Rect::new(0, 0, 15, 8), &theme);
        assert_eq!(inner.x, 0);
        assert_eq!(inner.width, 15);
    }

    #[test]
    fn boundary_exactly_20_cols_applies_padding() {
        let theme = Theme::paper_white();
        let (inner, _) = compute_inner_area(Rect::new(0, 0, 20, 8), &theme);
        assert_eq!(inner.x, 3);
        assert_eq!(inner.width, 14);
    }

    #[test]
    fn default_theme_reserves_no_chrome() {
        let theme = Theme::paper_white();
        let (inner, chrome) = compute_inner_area(Rect::new(0, 0, 80, 24), &theme);
        assert_eq!(inner.height, 24);
        assert_eq!(chrome.height, 0);
    }

    #[test]
    fn chrome_rows_reduce_inner_height() {
        let theme = Theme {
            chrome_rows: 2,
            ..Theme::paper_white()
        };
        let (inner, chrome) = compute_inner_area(Rect::new(0, 0, 80, 24), &theme);
        assert_eq!(inner.height, 22);
        assert_eq!(chrome.height, 2);
        assert_eq!(chrome.y, inner.y + inner.height);
    }

    #[test]
    fn chrome_rows_clamped_to_one_third_of_height() {
        let theme = Theme {
            chrome_rows: 100,
            ..Theme::paper_white()
        };
        let (inner, chrome) = compute_inner_area(Rect::new(0, 0, 80, 30), &theme);
        // height / 3 = 10
        assert_eq!(chrome.height, 10);
        assert_eq!(inner.height, 20);
    }

    #[test]
    fn honors_nonzero_outer_origin() {
        let theme = Theme::paper_white();
        let (inner, _) = compute_inner_area(Rect::new(5, 7, 80, 24), &theme);
        // pad_x = 6, so inner.x = 5 + 6 = 11.
        assert_eq!(inner.x, 11);
        assert_eq!(inner.y, 7);
    }

    #[test]
    fn zero_area_is_safe() {
        let theme = Theme::paper_white();
        let (inner, chrome) = compute_inner_area(Rect::new(0, 0, 0, 0), &theme);
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
        assert_eq!(chrome.height, 0);
    }

    // ----- is_hero_slide -----

    #[test]
    fn hero_h1_only() {
        let slide = slide_of(vec![cell_of(vec![heading_block(1, "Title")])]);
        assert!(is_hero_slide(&slide));
    }

    #[test]
    fn hero_h1_plus_paragraph() {
        let slide = slide_of(vec![cell_of(vec![
            heading_block(1, "Title"),
            paragraph_block("subtitle"),
        ])]);
        assert!(is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_h1_plus_list() {
        let slide = slide_of(vec![cell_of(vec![heading_block(1, "T"), list_block()])]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_h1_plus_code() {
        let slide = slide_of(vec![cell_of(vec![heading_block(1, "T"), code_block()])]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_h1_plus_image() {
        let slide = slide_of(vec![cell_of(vec![heading_block(1, "T"), image_block()])]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_paragraph_only() {
        let slide = slide_of(vec![cell_of(vec![paragraph_block("p")])]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_two_lists() {
        let slide = slide_of(vec![cell_of(vec![list_block(), list_block()])]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_h2_only() {
        let slide = slide_of(vec![cell_of(vec![heading_block(2, "Subtitle")])]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_paragraph_before_h1() {
        let slide = slide_of(vec![cell_of(vec![
            paragraph_block("intro"),
            heading_block(1, "Late"),
        ])]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_h1_plus_paragraph_plus_more() {
        let slide = slide_of(vec![cell_of(vec![
            heading_block(1, "T"),
            paragraph_block("p"),
            paragraph_block("p2"),
        ])]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_multi_cell_even_if_each_looks_hero() {
        let slide = slide_of(vec![
            cell_of(vec![heading_block(1, "A")]),
            cell_of(vec![heading_block(1, "B")]),
        ]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_empty_slide() {
        let slide = slide_of(vec![]);
        assert!(!is_hero_slide(&slide));
    }

    #[test]
    fn not_hero_empty_cell() {
        let slide = slide_of(vec![cell_of(vec![])]);
        assert!(!is_hero_slide(&slide));
    }

    // ----- render_hero vertical placement -----

    #[test]
    fn render_hero_centers_vertically_on_short_content() {
        let theme = Theme::paper_white();
        let slide = slide_of(vec![cell_of(vec![heading_block(1, "Hello")])]);
        let area = Rect::new(0, 0, 80, 24);
        let (inner, _) = compute_inner_area(area, &theme);
        let mut buf = Buffer::empty(area);
        render_hero(&slide, inner, &mut buf, &theme);

        // Find the row containing "Hello".
        let mut heading_row = None;
        for y in 0..area.height {
            let row: String = (0..area.width).map(|x| buf[(x, y)].symbol()).collect();
            if row.contains("Hello") {
                heading_row = Some(y);
                break;
            }
        }
        let y = heading_row.expect("heading must appear");
        // Expected top_offset = (24 - 1) / 2 = 11 (inner.y = 0, so row = 11).
        assert!((10..=12).contains(&y), "heading row was {}", y);
    }

    #[test]
    fn render_hero_anchors_top_when_short() {
        let theme = Theme::paper_white();
        let slide = slide_of(vec![cell_of(vec![heading_block(1, "Hi")])]);
        let area = Rect::new(0, 0, 40, 4);
        let (inner, _) = compute_inner_area(area, &theme);
        let mut buf = Buffer::empty(area);
        render_hero(&slide, inner, &mut buf, &theme);

        let row0: String = (0..area.width).map(|x| buf[(x, 0)].symbol()).collect();
        assert!(row0.contains("Hi"), "short area should anchor heading at top: {:?}", row0);
    }
}
