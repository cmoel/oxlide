pub mod ast;
pub mod fold;
pub mod prepass;

pub use ast::{
    Block, Cell, Directive, InlineSpan, ListItem, ParseError, Slide, SlideDeck, SourceSpan,
};

pub fn parse_deck(source: &str) -> Result<SlideDeck, ParseError> {
    let prepass_out = prepass::prepass(source);
    let mut deck = fold::fold(&prepass_out);
    deck.source = source.to_string();
    Ok(deck)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(source: &str) -> SlideDeck {
        parse_deck(source).expect("parse_deck should not fail in v1")
    }

    #[test]
    fn acceptance_two_slides_with_rule() {
        let src = "# Hello\n\nWorld\n\n---\n\n# Two";
        let d = deck(src);
        assert_eq!(d.slides.len(), 2);
        assert_eq!(d.slides[0].cells.len(), 2, "heading + paragraph as 2 cells");
        assert!(matches!(
            d.slides[0].cells[0].blocks[0],
            Block::Heading { level: 1, .. }
        ));
        assert!(matches!(
            d.slides[0].cells[1].blocks[0],
            Block::Paragraph { .. }
        ));
        assert_eq!(d.slides[1].cells.len(), 1);
        assert!(matches!(
            d.slides[1].cells[0].blocks[0],
            Block::Heading { level: 1, .. }
        ));
    }

    #[test]
    fn acceptance_two_slides_double_blank() {
        let d = deck("# A\n\n\n\n# B");
        assert_eq!(d.slides.len(), 2);
    }

    #[test]
    fn acceptance_cell_break_single_blank() {
        let d = deck("Para A\n\nPara B");
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 2);
    }

    #[test]
    fn empty_input_produces_empty_deck() {
        let d = deck("");
        assert!(d.slides.is_empty());
        assert_eq!(d.source, "");
    }

    #[test]
    fn only_blank_lines_produces_empty_deck() {
        let d = deck("\n\n\n");
        assert!(d.slides.is_empty());
    }

    #[test]
    fn source_preserved_on_deck() {
        let src = "# Hello";
        let d = deck(src);
        assert_eq!(d.source, src);
    }

    #[test]
    fn heading_levels_one_to_six() {
        for (i, prefix) in ["#", "##", "###", "####", "#####", "######"]
            .iter()
            .enumerate()
        {
            let src = format!("{} heading", prefix);
            let d = deck(&src);
            match &d.slides[0].cells[0].blocks[0] {
                Block::Heading { level, .. } => assert_eq!(*level as usize, i + 1),
                other => panic!("expected heading, got {:?}", other),
            }
        }
    }

    #[test]
    fn spans_reference_original_source_byte_offsets() {
        let src = "# A\n\n\n\n# B";
        let d = deck(src);
        assert_eq!(d.slides.len(), 2);
        let h1 = &d.slides[0].cells[0].blocks[0];
        if let Block::Heading { span, .. } = h1 {
            assert_eq!(&src[span.start..span.end].trim_end(), &"# A");
        } else {
            panic!("expected heading");
        }
        let h2 = &d.slides[1].cells[0].blocks[0];
        if let Block::Heading { span, .. } = h2 {
            assert_eq!(&src[span.start..span.end].trim_end(), &"# B");
        } else {
            panic!("expected heading");
        }
    }

    #[test]
    fn nested_list_preserves_structure() {
        let src = "- outer\n  - inner\n";
        let d = deck(src);
        let outer = &d.slides[0].cells[0].blocks[0];
        if let Block::List {
            ordered: false,
            items,
            ..
        } = outer
        {
            assert_eq!(items.len(), 1);
            let outer_item = &items[0];
            let inner = outer_item
                .blocks
                .iter()
                .find(|b| matches!(b, Block::List { .. }));
            assert!(inner.is_some(), "outer item should contain nested list");
        } else {
            panic!("expected outer unordered list, got {:?}", outer);
        }
    }

    #[test]
    fn single_slide_when_no_break() {
        let d = deck("# heading\n\npara\n\npara2\n");
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 3);
    }

    #[test]
    fn consecutive_rules_do_not_emit_empty_slides() {
        let d = deck("# A\n\n---\n\n---\n\n# B");
        assert_eq!(d.slides.len(), 2);
    }

    #[test]
    fn fixture_multi_slide_rule() {
        let src = include_str!("fixtures/multi-slide-rule.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 2);
    }

    #[test]
    fn fixture_multi_slide_double_blank() {
        let src = include_str!("fixtures/multi-slide-double-blank.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 2);
    }

    #[test]
    fn fixture_cell_break_single_blank() {
        let src = include_str!("fixtures/cell-break-single-blank.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 2);
    }

    #[test]
    fn fixture_single_slide_paragraph() {
        let src = include_str!("fixtures/single-slide-paragraph.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert_eq!(d.slides[0].cells.len(), 1);
    }

    #[test]
    fn fixture_single_slide_heading() {
        let src = include_str!("fixtures/single-slide-heading.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
        assert!(matches!(
            d.slides[0].cells[0].blocks[0],
            Block::Heading { .. }
        ));
    }

    #[test]
    fn fixture_nested_list() {
        let src = include_str!("fixtures/nested-list.md");
        let d = deck(src);
        assert!(matches!(d.slides[0].cells[0].blocks[0], Block::List { .. }));
    }

    #[test]
    fn fixture_ordered_list() {
        let src = include_str!("fixtures/ordered-list.md");
        let d = deck(src);
        if let Block::List { ordered, .. } = &d.slides[0].cells[0].blocks[0] {
            assert!(ordered);
        } else {
            panic!("expected ordered list");
        }
    }

    #[test]
    fn fixture_empty_input() {
        let src = include_str!("fixtures/empty-input.md");
        let d = deck(src);
        assert!(d.slides.is_empty());
    }

    #[test]
    fn fixture_leading_trailing_rule() {
        let src = include_str!("fixtures/leading-trailing-rule.md");
        let d = deck(src);
        assert_eq!(d.slides.len(), 1);
    }
}
